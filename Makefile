.PHONY: build release run run-file test clean bundle

# ── Build ────────────────────────────────────────────────────────

build:
	cargo build

release:
	cargo build --release

# ── Run ──────────────────────────────────────────────────────────

run:
	cargo run

run-file:
	cargo run -- $(FILE)

# ── Test ─────────────────────────────────────────────────────────

test:
	cargo test

# ── Install (local user, no root) ────────────────────────────────

prefix ?= $(HOME)/.local
install: release
	mkdir -p $(prefix)/bin $(prefix)/share/applications $(prefix)/share/man/man1 $(prefix)/share/icons/hicolor/scalable/apps
	cp target/release/muT $(prefix)/bin/muT
	cp resources/muT.desktop $(prefix)/share/applications/muT.desktop
	cp resources/muT.1 $(prefix)/share/man/man1/muT.1
	cp resources/muT.svg $(prefix)/share/icons/hicolor/scalable/apps/muT.svg
	@echo "Installed to $(prefix).  Ensure $(prefix)/bin is in your PATH."

# ── Package (AUR style, requires makepkg) ───────────────────────

package: PKGBUILD .SRCINFO resources/muT.desktop resources/muT.1
	makepkg -f

# ── Maintenance ──────────────────────────────────────────────────

clean:
	cargo clean

# ── TeX bundle ───────────────────────────────────────────────────

# The tectonic support bundle (~30 MB) is auto-downloaded on the
# first Ctrl+B and cached at ~/.cache/Tectonic/.  No manual setup
# is required.  Run `make build` first, then start the editor.
