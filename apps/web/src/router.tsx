import { createRootRoute, createRoute, createRouter, lazyRouteComponent } from '@tanstack/react-router'

import { RootRoute } from './routes/__root'

const rootRoute = createRootRoute({ component: RootRoute })
const landingRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: lazyRouteComponent(() => import('./routes/landing'), 'LandingPage'),
})
const queueRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/issues',
  component: lazyRouteComponent(() => import('./routes/queue'), 'WorkspaceQueuePage'),
})
const issueDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/teams/$teamKey/issues/$issueId',
  component: lazyRouteComponent(() => import('./routes/issue-detail'), 'IssueDetailPage'),
})
const legacyIssueDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/issues/$issueId',
  component: lazyRouteComponent(() => import('./routes/issue-detail'), 'LegacyIssueDetailRedirect'),
})
const agentsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/agents',
  component: lazyRouteComponent(() => import('./routes/agents'), 'AgentsPage'),
})
const integrationsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/integrations',
  component: lazyRouteComponent(() => import('./routes/integrations'), 'IntegrationsPage'),
})
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/settings',
  component: lazyRouteComponent(() => import('./routes/settings'), 'SettingsPage'),
})
const loginRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/login',
  component: lazyRouteComponent(() => import('./routes/login'), 'LoginPage'),
})
const approvalsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/approvals',
  component: lazyRouteComponent(() => import('./routes/approvals'), 'ApprovalsPage'),
})
const inboxRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/inbox',
  component: lazyRouteComponent(() => import('./routes/inbox'), 'InboxPage'),
})
const triageRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/triage',
  component: lazyRouteComponent(() => import('./routes/triage'), 'TriagePage'),
})
const teamIssuesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/teams/$teamKey/issues',
  component: lazyRouteComponent(() => import('./routes/team-issues'), 'TeamIssuesPage'),
})
const teamResourceRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/teams/$teamKey',
  component: lazyRouteComponent(() => import('./routes/team-resource'), 'TeamResourcePage'),
})
const projectResourceRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/projects/$projectId',
  component: lazyRouteComponent(() => import('./routes/project-resource'), 'ProjectResourcePage'),
})
const teamSettingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/teams/$teamKey/settings',
  component: lazyRouteComponent(() => import('./routes/team-settings'), 'TeamSettingsPage'),
})
const onboardingRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/onboarding',
  component: lazyRouteComponent(() => import('./routes/onboarding'), 'OnboardingPage'),
})
const documentRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/documents/$documentId',
  component: lazyRouteComponent(() => import('./routes/document'), 'DocumentPage'),
})
const organizationDocumentsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/$organizationSlug/documents',
  component: lazyRouteComponent(() => import('./routes/organization-documents'), 'OrganizationDocumentsPage'),
})

const routeTree = rootRoute.addChildren([landingRoute, queueRoute, triageRoute, teamResourceRoute, projectResourceRoute, teamSettingsRoute, teamIssuesRoute, onboardingRoute, organizationDocumentsRoute, documentRoute, issueDetailRoute, legacyIssueDetailRoute, agentsRoute, integrationsRoute, settingsRoute, loginRoute, approvalsRoute, inboxRoute])

export const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}
