# cosmic-comp-rdp

Compositor for the [COSMIC Desktop Environment](https://github.com/pop-os/cosmic-epoch) with RemoteDesktop EIS (Emulated Input Server) support.

This is a fork of [pop-os/cosmic-comp](https://github.com/pop-os/cosmic-comp) that adds the ability to receive input events from remote desktop sessions. The EIS receiver allows the [xdg-desktop-portal-cosmic](https://github.com/olafkfreund/xdg-desktop-portal-cosmic) RemoteDesktop portal to inject keyboard, mouse, and touch input into the compositor on behalf of RDP clients.

Part of the [COSMIC Remote Desktop stack](#full-remote-desktop-stack) - works together with [cosmic-rdp-server](https://github.com/olafkfreund/cosmic-rdp-server) (RDP daemon) and [xdg-desktop-portal-cosmic](https://github.com/olafkfreund/xdg-desktop-portal-cosmic) (portal).

## Features

- **Full COSMIC compositor functionality** (window management, workspaces, tiling, animations, etc.)
- **EIS input receiver** via D-Bus (`com.system76.CosmicComp.RemoteDesktop`)
- **Keyboard injection** from remote sessions (full scancode support)
- **Pointer injection** (relative motion, absolute position, buttons, scroll)
- **Touch injection** (multi-touch down, motion, up, cancel)
- **Per-session isolation** via UNIX socket pairs managed by the portal
- **Calloop integration** for non-blocking event processing in the compositor event loop

## Architecture

### EIS Input Receiver

The compositor exposes a D-Bus interface for accepting EIS socket file descriptors from the portal:

```
Bus Name:  com.system76.CosmicComp.RemoteDesktop
Object:    /com/system76/CosmicComp
Interface: com.system76.CosmicComp.RemoteDesktop
Method:    AcceptEisSocket(fd: OwnedFd)
```

### How it works

```
xdg-desktop-portal-cosmic                    cosmic-comp-rdp
        |                                          |
        |  AcceptEisSocket(server_fd) via D-Bus    |
        |----------------------------------------->|
        |                                          |
        |                              EIS handshake (server side)
        |                              Create seat: keyboard + pointer + touch
        |                                          |
        |                              Register calloop event source
        |                                          |
   [client_fd returned to RDP server]              |
        |                                          |
   Input events via EIS protocol                   |
        |----------------------------------------->|
        |                              Inject into Smithay input pipeline
        |                              (indistinguishable from local hardware)
```

1. The xdg-desktop-portal-cosmic RemoteDesktop portal creates a UNIX socket pair during `Start`
2. The server-side fd is sent to the compositor via `AcceptEisSocket`
3. The compositor performs the EIS handshake (server side) and creates a seat with keyboard, pointer, and touch capabilities
4. Input events from the remote client are injected into Smithay's input pipeline
5. Injected events are indistinguishable from local hardware input

### Input events supported

| Event | Description |
|-------|-------------|
| `KeyboardKey` | Key press/release with evdev keycodes |
| `PointerMotion` | Relative mouse movement (dx, dy) |
| `PointerMotionAbsolute` | Absolute mouse position (x, y) |
| `Button` | Mouse button press/release (left, right, middle, etc.) |
| `ScrollDelta` | Smooth scroll (dx, dy) |
| `ScrollDiscrete` | Discrete scroll steps |
| `TouchDown` | Touch point placed (multi-touch id, x, y) |
| `TouchMotion` | Touch point moved (multi-touch id, x, y) |
| `TouchUp` | Touch point lifted (multi-touch id) |
| `TouchCancel` | Touch sequence cancelled |

### Key source files

| File | Purpose |
|------|---------|
| `src/input/eis.rs` | EIS receiver: protocol handling, event routing, Smithay injection |
| `src/dbus/eis.rs` | D-Bus interface for accepting EIS socket fds from the portal |

## Requirements

- **Rust 1.90+** (edition 2024)
- **Wayland** development headers
- **libxkbcommon**, **libinput**, **libei**
- **Mesa** (GPU acceleration)
- **seatd** (seat management)
- **systemd**, **fontconfig**, **pixman**, **libdisplay-info**

## Building

### Using Nix (recommended)

```bash
nix develop              # Enter dev shell with all dependencies
cargo build --release    # Build release binary

# Or build directly with Nix
nix build
```

### Using Cargo (requires system libraries)

Install the required development headers for your distribution:

**Fedora/RHEL:**
```bash
sudo dnf install wayland-devel libxkbcommon-devel libinput-devel mesa-libEGL-devel \
  mesa-libGL-devel seatd-devel libei-devel systemd-devel fontconfig-devel \
  pixman-devel libdisplay-info-devel clang-devel
```

**Debian/Ubuntu:**
```bash
sudo apt install libwayland-dev libxkbcommon-dev libinput-dev libegl-dev \
  libgl-dev libseat-dev libei-dev libsystemd-dev libfontconfig-dev \
  libpixman-1-dev libdisplay-info-dev clang
```

**Arch Linux:**
```bash
sudo pacman -S wayland libxkbcommon libinput mesa seatd libei systemd \
  fontconfig pixman libdisplay-info clang
```

Then build:
```bash
cargo build --release
```

### Rust version

Requires Rust 1.90+ (edition 2024). The `rust-toolchain.toml` file specifies the exact version.

### Building an AUR package (Arch Linux)

Create a `PKGBUILD`:

```bash
# Maintainer: Your Name <you@example.com>
pkgname=cosmic-comp-rdp
pkgver=1.0.0
pkgrel=1
pkgdesc="COSMIC compositor with RemoteDesktop EIS support"
arch=('x86_64' 'aarch64')
url="https://github.com/olafkfreund/cosmic-comp-rdp"
license=('GPL-3.0-only')
depends=('wayland' 'libxkbcommon' 'libinput' 'mesa' 'seatd' 'libei'
         'systemd' 'fontconfig' 'pixman' 'libdisplay-info')
makedepends=('cargo' 'clang' 'pkg-config')
provides=('cosmic-comp')
conflicts=('cosmic-comp')
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

prepare() {
  cd "$pkgname-$pkgver"
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd "$pkgname-$pkgver"
  export RUSTUP_TOOLCHAIN=stable
  cargo build --release
}

package() {
  cd "$pkgname-$pkgver"
  install -Dm0755 "target/release/cosmic-comp" "$pkgdir/usr/bin/cosmic-comp"
}
```

Build and install:
```bash
makepkg -si
```

### Building a Debian package

Create the `debian/` directory structure:

```bash
mkdir -p debian/source
```

**`debian/control`:**
```
Source: cosmic-comp-rdp
Section: x11
Priority: optional
Maintainer: Your Name <you@example.com>
Build-Depends: debhelper-compat (= 13), cargo, rustc (>= 1.90),
 clang, pkg-config, libwayland-dev, libxkbcommon-dev, libinput-dev,
 libegl-dev, libgl-dev, libseat-dev, libei-dev, libsystemd-dev,
 libfontconfig-dev, libpixman-1-dev, libdisplay-info-dev
Standards-Version: 4.7.0
Homepage: https://github.com/olafkfreund/cosmic-comp-rdp

Package: cosmic-comp-rdp
Architecture: any
Depends: ${shlibs:Depends}, ${misc:Depends}
Provides: cosmic-comp
Conflicts: cosmic-comp
Description: COSMIC compositor with RemoteDesktop EIS support
 Fork of the COSMIC desktop compositor that adds an EIS input
 receiver for remote desktop sessions. Allows the RemoteDesktop
 portal to inject keyboard, mouse, and touch input via libei.
```

**`debian/rules`:**
```makefile
#!/usr/bin/make -f
%:
	dh $@

override_dh_auto_build:
	cargo build --release

override_dh_auto_install:
	install -Dm0755 target/release/cosmic-comp debian/cosmic-comp-rdp/usr/bin/cosmic-comp
```

**`debian/changelog`:**
```
cosmic-comp-rdp (1.0.0-1) unstable; urgency=medium

  * Initial release with EIS input receiver support.

 -- Your Name <you@example.com>  Mon, 10 Feb 2026 00:00:00 +0000
```

**`debian/source/format`:**
```
3.0 (quilt)
```

Build the package:
```bash
dpkg-buildpackage -us -uc -b
```

## Installation

### NixOS Module

The flake provides a NixOS module for declarative configuration.

#### Basic setup

```nix
{
  inputs.cosmic-comp.url = "github:olafkfreund/cosmic-comp-rdp";

  outputs = { self, nixpkgs, cosmic-comp, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      modules = [
        cosmic-comp.nixosModules.default
        {
          nixpkgs.overlays = [ cosmic-comp.overlays.default ];

          services.cosmic-comp = {
            enable = true;
            eis.enable = true;  # enabled by default

            settings = {
              xkb-config = {
                layout = "us";
              };
            };
          };
        }
      ];
    };
  };
}
```

#### NixOS module options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enable` | bool | `false` | Enable the COSMIC compositor |
| `package` | package | `pkgs.cosmic-comp` | Compositor package to use |
| `eis.enable` | bool | `true` | Enable the EIS D-Bus interface for remote input |
| `settings` | attrs | `{}` | Compositor configuration (freeform TOML) |
| `settings.xkb-config.layout` | string | `"us"` | XKB keyboard layout |
| `settings.xkb-config.variant` | string | `""` | XKB layout variant |
| `settings.xkb-config.options` | string | `""` | XKB options (e.g., `ctrl:nocaps`) |
| `settings.xkb-config.model` | string | `""` | XKB keyboard model |

The module automatically enables:
- `hardware.graphics` (GPU acceleration)
- `services.seatd` (seat management)
- `security.polkit` (device access)
- D-Bus registration for EIS (when `eis.enable = true`)

### Home Manager Module

For user-level configuration of the compositor.

```nix
{
  inputs.cosmic-comp.url = "github:olafkfreund/cosmic-comp-rdp";

  outputs = { self, nixpkgs, home-manager, cosmic-comp, ... }: {
    homeConfigurations."user" = home-manager.lib.homeManagerConfiguration {
      modules = [
        cosmic-comp.homeManagerModules.default
        {
          nixpkgs.overlays = [ cosmic-comp.overlays.default ];

          wayland.compositor.cosmic-comp = {
            enable = true;

            xkb = {
              layout = "de";
              variant = "nodeadkeys";
              options = "ctrl:nocaps";
            };
          };
        }
      ];
    };
  };
}
```

#### Home Manager options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enable` | bool | `false` | Enable the COSMIC compositor |
| `package` | package | `pkgs.cosmic-comp` | Compositor package to use |
| `xkb.layout` | string | `"us"` | XKB keyboard layout |
| `xkb.variant` | string | `""` | XKB layout variant |
| `xkb.options` | string | `""` | XKB options |
| `xkb.model` | string | `""` | XKB keyboard model |
| `extraConfig` | attrs | `{}` | Additional cosmic-config settings |

The Home Manager module writes XKB configuration to `~/.config/cosmic-comp/v1/xkb-config` when non-default keyboard settings are specified.

### Manual installation

After building:
```bash
sudo install -Dm0755 target/release/cosmic-comp /usr/bin/cosmic-comp
```

## Full Remote Desktop Stack

For a complete remote desktop setup, you need all three components:

```
                                    +-----------------------+
                                    |  cosmic-comp-rdp      |
                                    |  (this repo)          |
                                    +-----------^-----------+
                                                |
                                    AcceptEisSocket(fd)
                                                |
+------------+     +-------------------+     +--+--------------------------+
| RDP Client | --> | cosmic-rdp-server | --> | xdg-desktop-portal-cosmic   |
| (mstsc,    |     | (RDP daemon)      |     | (RemoteDesktop + ScreenCast)|
| FreeRDP,   | <-- | RDP protocol,     | <-- | EIS socket pairs,          |
| Remmina)   |     | TLS, auth         |     | PipeWire streams            |
+------------+     +-------------------+     +-----------------------------+
```

| Component | Repository | Purpose |
|-----------|-----------|---------|
| [cosmic-rdp-server](https://github.com/olafkfreund/cosmic-rdp-server) | RDP daemon | RDP protocol server, capture + input orchestration |
| [xdg-desktop-portal-cosmic](https://github.com/olafkfreund/xdg-desktop-portal-cosmic) | Portal fork | RemoteDesktop + ScreenCast portal interfaces |
| [cosmic-comp-rdp](https://github.com/olafkfreund/cosmic-comp-rdp) | This repo | EIS receiver for input injection |

### NixOS example (all three components)

```nix
{
  inputs = {
    cosmic-rdp-server.url = "github:olafkfreund/cosmic-rdp-server";
    xdg-desktop-portal-cosmic.url = "github:olafkfreund/xdg-desktop-portal-cosmic";
    cosmic-comp.url = "github:olafkfreund/cosmic-comp-rdp";
  };

  outputs = { self, nixpkgs, cosmic-rdp-server, xdg-desktop-portal-cosmic, cosmic-comp, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      modules = [
        cosmic-rdp-server.nixosModules.default
        xdg-desktop-portal-cosmic.nixosModules.default
        cosmic-comp.nixosModules.default
        {
          nixpkgs.overlays = [
            cosmic-rdp-server.overlays.default
            xdg-desktop-portal-cosmic.overlays.default
            cosmic-comp.overlays.default
          ];

          # Compositor with EIS support
          services.cosmic-comp.enable = true;

          # Portal with RemoteDesktop interface
          services.xdg-desktop-portal-cosmic.enable = true;

          # RDP server
          services.cosmic-rdp-server = {
            enable = true;
            openFirewall = true;
            settings.bind = "0.0.0.0:3389";
          };
        }
      ];
    };
  };
}
```

### Component compatibility

All three repositories use compatible dependency versions:

| Dependency | cosmic-rdp-server | xdg-desktop-portal-cosmic | cosmic-comp-rdp |
|------------|-------------------|---------------------------|-----------------|
| reis (libei) | 0.5 | 0.5 | 0.5 |
| zbus (D-Bus) | 5.x | 5.x | 5.x |

D-Bus interface chain:
- RDP server calls portal `org.freedesktop.impl.portal.RemoteDesktop` with `ConnectToEIS`
- Portal calls compositor `com.system76.CosmicComp.RemoteDesktop.AcceptEisSocket(fd)`
- Compositor creates EIS seat and begins receiving input events

## Related Projects

| Project | Description |
|---------|-------------|
| [cosmic-rdp-server](https://github.com/olafkfreund/cosmic-rdp-server) | RDP server daemon using the portal for capture and input |
| [xdg-desktop-portal-cosmic](https://github.com/olafkfreund/xdg-desktop-portal-cosmic) | Portal backend with RemoteDesktop interface |
| [cosmic-epoch](https://github.com/pop-os/cosmic-epoch) | COSMIC Desktop Environment |
| [cosmic-comp](https://github.com/pop-os/cosmic-comp) | Upstream COSMIC compositor |

## License

GPL-3.0-only
