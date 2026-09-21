# M0-50 Ephemeral window lifetime

Date: 2026-09-16  
Status: in-progress  
Requirement: EPROFILE-003

## Implemented slice

Secondary windows, including windows created by the ephemeral hint target,
intercept close requests. Active downloads are tracked in the owning window;
the user can keep the window open or cancel its downloads before closing.
After confirmation, the window requests reducer shutdown, marks the close as
approved, and destroys its views and `BrowserUi` state. Primary-window shutdown
uses the same download confirmation boundary.

The Rust owner cleans the exact temporary root after the GUI exits. Ephemeral
windows use an off-the-record profile, so the Qt profile and memory-only Rust
state are released with the window. Shared-profile multi-window owner tokens,
capture/prompt qualification, and explicit final-owner accounting remain later
slices.

## Verification

Qt regression coverage asserts the transient profile and close/shutdown
wiring. The repository verifier and native Wayland smoke are the required
integration gates.
