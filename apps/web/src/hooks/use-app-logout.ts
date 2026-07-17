import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { logout } from "@/lib/api";

export function useAppLogout() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  return async () => {
    await logout();
    const [{ LoroDocumentSession }, { clearLoroDocumentPersistence }, { clearSessionCollections }] = await Promise.all([
      import("@/lib/loro-document"),
      import("@/lib/loro-persistence"),
      import("@/lib/session-state"),
    ]);
    await LoroDocumentSession.disposeAll();
    await clearSessionCollections();
    await clearLoroDocumentPersistence();
    queryClient.clear();
    await navigate({ to: "/login", replace: true });
  };
}
