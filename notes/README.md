# Riichi working notes

These notes separate product decisions from implementation research.

The current source plan mixes four kinds of material:

1. product strategy and the wedge;
2. the pilot product needed to validate that wedge;
3. the later MVP product after validation;
4. technical RFC topics and architectural alternatives.

The first completed artifact is the [pilot PRD](./riichi-pilot-prd.md). The [pilot architecture RFC](./riichi-pilot-architecture-rfc.md) translates its correctness requirements into an implementation boundary. The [document and metadata sync RFC](./riichi-write-boundary-sync-rfc.md) defines the post-pilot write boundary between Loro documents and server-arbitrated metadata. These notes remain intentionally separate from later product and technology work:

The [familiarity and interaction optimization RFC](./riichi-familiarity-optimization-rfc.md) defines the bounded UX pass for teams arriving from Linear-like issue trackers. It improves navigation, queue ergonomics, keyboard use, and feedback without introducing cycles or timeboxing.

The [post-pilot product direction RFC](./riichi-post-pilot-product-direction-rfc.md) extends that direction into a feature order for the full product. It prioritizes operating existing agent state through previews, shared views, relationships, history, dates, workflows, templates, and project visibility before adding heavyweight planning features.

The [human CLI RFC](./riichi-human-cli-rfc.md) defines the slices for making the existing agent CLI easier for people to use without weakening the machine-facing CLI or MCP contract.

- product brief: why Riichi exists, who it serves, and how it differs;
- MVP PRD: the complete human-and-agent product after the pilot;
- later technical RFCs: deeper leases and fencing, context construction, tenancy and auth, GitHub integration, and audit/outbox.

The original plan in `/Users/natalie/Downloads/riichi-revised-plan(1).md` is preserved as source material.
