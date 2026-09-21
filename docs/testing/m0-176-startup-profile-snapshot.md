# M0-176: startup profile snapshot reuse

Profile bootstrap now publishes the durable profile-name snapshot obtained
while opening the selected profile store. Startup context-definition validation
reuses that snapshot instead of reopening the profile registry on the Qt
thread. This removes the duplicate registry read and preserves missing-profile
diagnostics before context membership is published.
