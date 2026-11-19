#!/bin/bash
# scripts/fresco-update.sh picks the .deb by the installed package: fresco_*.deb
# normally, fresco-legacy_*.deb when fresco-legacy is installed. Fakes apt-get,
# dpkg-query, dpkg-deb and curl on PATH (no network, no root) and checks which
# asset the updater downloads and hands to apt.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SCRIPT="$ROOT/scripts/fresco-update.sh"
RELEASE_JSON='{"tag_name": "v1.1.56", "assets": [
 {"browser_download_url": "https://example.test/dl/v1.1.56/fresco_1.1.56-1_amd64.deb"},
 {"browser_download_url": "https://example.test/dl/v1.1.56/fresco-legacy_1.1.56-1_amd64.deb"},
 {"browser_download_url": "https://example.test/dl/v1.1.56/fresco-1.1.56-1.x86_64.rpm"}
]}'

# run_case <legacy-installed: 0|1> <expected asset file name>
run_case() {
    local legacy="$1" expect="$2" tmp
    tmp="$(mktemp -d)"
    mkdir -p "$tmp/bin"

    cat > "$tmp/bin/dpkg-query" <<EOF
#!/bin/sh
# -W -f='\${Status}' <pkg> | -f='\${Version}' <pkg>
case "\$*" in
  *Status*fresco-legacy*) [ "$legacy" = 1 ] && echo 'install ok installed' && exit 0; exit 1 ;;
  *Status*) exit 1 ;;
  *Version*) echo '1.1.55-1' ;;
esac
EOF
    # The update's own version check (dpkg --compare-versions) is real dpkg.
    cat > "$tmp/bin/apt-get" <<EOF
#!/bin/sh
echo "apt-get \$*" >> "$tmp/apt.log"
EOF
    cat > "$tmp/bin/dpkg-deb" <<'EOF'
#!/bin/sh
exit 0
EOF
    cat > "$tmp/bin/curl" <<EOF
#!/bin/sh
url=""; out=""; head=0
while [ \$# -gt 0 ]; do
  case "\$1" in
    -*I*) head=1 ;;
    -o) out="\$2"; shift ;;
    http*) url="\$1" ;;
  esac
  shift
done
case "\$url" in
  *api.github.com*) printf '%s\n' '$RELEASE_JSON' ;;
  *) echo "\$url" >> "$tmp/downloads.log"
     if [ "\$head" = 1 ]; then printf 'content-length: 16\r\n'; else printf 'fake deb' > "\$out"; fi ;;
esac
EOF
    chmod +x "$tmp"/bin/*

    PATH="$tmp/bin:$PATH" FRESCO_REPO=example/fresco bash "$SCRIPT" >"$tmp/out.log" 2>&1 || {
        cat "$tmp/out.log"; echo "FAIL: updater exited non-zero (legacy=$legacy)"; exit 1; }

    local got
    got="$(basename "$(tail -1 "$tmp/downloads.log")")"
    if [ "$got" != "$expect" ]; then
        cat "$tmp/out.log"; echo "FAIL: legacy=$legacy downloaded '$got', want '$expect'"; exit 1
    fi
    echo "ok: legacy=$legacy -> $got"
    rm -rf "$tmp"
}

run_case 0 fresco_1.1.56-1_amd64.deb
run_case 1 fresco-legacy_1.1.56-1_amd64.deb
echo "updater deb asset selection: ok"
