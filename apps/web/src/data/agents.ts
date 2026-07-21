import type { CreatedAgentSession } from "@/lib/api";

export function agentCliCommand(projectId: string, session: CreatedAgentSession) {
  return `RIICHI_PROJECT_ID=${projectId} RIICHI_SESSION_ID=${session.session_id} RIICHI_AGENT_TOKEN=${session.agent_token} riichi-agent ready`;
}
