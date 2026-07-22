# Riichi frontend design contract

## Audience

Riichi is used primarily by project owners and people operating the everyday
issue queue. They are making quick decisions about what needs attention, what
an agent is doing, and what action is safe next.

## Product promise

The interface should feel calm and quick. A user should be able to understand
the current state, take the next authorized action, and return to the queue
without surprises.

## Visual direction

- Dark-only, using the existing CSS token system in `apps/web/src/index.css`.
- Quiet, utilitarian, and precise. Contrast and hierarchy carry emphasis.
- Avoid decorative gradients, glowing effects, dense dashboard chrome, and
  generic card grids.
- Prefer clear text labels for important actions; icons support recognition but
  do not carry authority alone.
- Use the existing Instrument Sans, Valley Sans, and Ioskeley Mono roles
  consistently rather than introducing another font family.

## Interaction principles

1. Show the current state and the next safe action together.
2. Preserve queue context when opening previews, details, or configuration.
3. Make loading, success, failure, and permission states explicit.
4. Keep destructive or authority-changing actions deliberate and reversible
   where the domain permits it.
5. Make keyboard and pointer paths equivalent for core queue operations.
6. Design narrow screens as a first-class operating context, not a shrunken
   desktop layout.

## Quality bar

Every frontend change should be checked for WCAG AA contrast, visible focus,
44px touch targets where practical, reduced motion, empty/loading/error states,
and consistency with existing components and tokens.
