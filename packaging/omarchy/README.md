# Ferric Browser Omarchy integration

This is an optional, community-maintained integration package. It is not an
Omarchy project component and does not claim Omarchy endorsement.

The package installs only vendor-owned resources under `/usr/share/ferric-browser-omarchy`
plus three small command wrappers under `/usr/bin`. The wrappers use the public
`ferric-browser query switcher` and `ferric-browser activate tab` interfaces; they do
not read browser databases or scrape window titles.

User activation is deliberately explicit:

1. Review the installed template and the versioned Hyprland examples. Use the
   `.conf` example for Hyprland 0.51–0.54 and the `.lua` example for 0.55+.
2. Copy or include only the specific resource you want in your user-owned
   configuration.
3. Adjust the copied file for the installed Omarchy and Hyprland versions.

Upgrades replace only package-owned files. They never edit `~/.config`,
`~/.local`, Hyprland configuration, launcher configuration, or Ferric Browser
configuration. The doctor reports missing optional capabilities without
changing the system.
