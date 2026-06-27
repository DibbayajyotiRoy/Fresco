import { Download as DownloadIcon, Package, Terminal } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { CopyButton } from "@/components/copy-button";
import { APT_INSTALL, INSTALL_ONELINER, RELEASES_URL } from "@/lib/site";

function CodeSnippet({ code }: { code: string }) {
  return (
    <div className="flex items-start gap-2 rounded-lg border border-border/70 bg-background/60 px-3 py-2">
      <code className="min-w-0 flex-1 whitespace-pre-wrap [overflow-wrap:anywhere] font-mono text-xs leading-relaxed text-muted-foreground">
        {code}
      </code>
      <CopyButton value={code} />
    </div>
  );
}

export function Download() {
  return (
    <section id="download" className="border-b border-border/60 py-20 sm:py-28">
      <div className="mx-auto max-w-6xl px-5">
        <div className="flex flex-col items-start justify-between gap-4 sm:flex-row sm:items-end">
          <div className="max-w-2xl">
            <p className="text-sm font-medium text-ink-subtle">Download</p>
            <h2 className="mt-2 text-3xl font-semibold tracking-tight text-ink sm:text-4xl">
              Install on Debian, Ubuntu, Pop!_OS &amp; Mint.
            </h2>
          </div>
          <Badge variant="outline" className="border-border text-ink-subtle">
            X11 and Wayland
          </Badge>
        </div>

        <div className="mt-12 grid gap-5 lg:grid-cols-2">
          {/* .deb / GitHub releases */}
          <Card className="flex flex-col p-7 shadow-none">
            <div className="flex items-center gap-3">
              <div className="flex size-10 items-center justify-center rounded-lg border border-border bg-surface-2 text-ink">
                <Package className="size-5" />
              </div>
              <div>
                <h3 className="font-semibold tracking-tight text-ink">
                  Download the .deb
                </h3>
                <p className="text-xs text-ink-subtle">
                  From GitHub releases
                </p>
              </div>
            </div>
            <p className="mt-4 text-sm text-ink-subtle">
              Grab the latest <code className="font-mono text-xs">.deb</code>{" "}
              and double-click it, or install from the terminal:
            </p>
            <div className="mt-4 space-y-2">
              <CodeSnippet code={APT_INSTALL} />
            </div>
            <p className="mt-4 text-xs text-muted-foreground">
              Prefer one command? Run the installer:
            </p>
            <div className="mt-2">
              <CodeSnippet code={INSTALL_ONELINER} />
            </div>
            <div className="mt-6">
              <Button asChild className="w-full font-medium sm:w-auto">
                <a
                  href={RELEASES_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  <DownloadIcon />
                  Latest release
                </a>
              </Button>
            </div>
          </Card>

          {/* Flathub, not published yet */}
          <Card className="flex flex-col p-7 shadow-none">
            <div className="flex items-center gap-3">
              <div className="flex size-10 items-center justify-center rounded-lg border border-border bg-surface-2 text-ink-subtle">
                <Terminal className="size-5" />
              </div>
              <div>
                <h3 className="font-semibold tracking-tight text-ink">Flathub</h3>
                <p className="text-xs text-ink-subtle">
                  Sandboxed Flatpak build
                </p>
              </div>
              <Badge variant="secondary" className="ml-auto">
                Coming soon
              </Badge>
            </div>
            <p className="mt-4 text-sm text-muted-foreground">
              A Flatpak build with automatic updates is in the works. For now,
              grab the <code className="font-mono text-xs">.deb</code> from
              GitHub releases.
            </p>
            <p className="mt-4 text-xs text-muted-foreground">
              For the lowest CPU usage, install your GPU&apos;s hardware-decode
              driver (Intel media VA driver, Mesa VA drivers, or the NVIDIA
              proprietary driver for NVDEC).
            </p>
            <div className="mt-auto pt-6">
              <Button
                variant="outline"
                className="w-full font-medium sm:w-auto"
                disabled
              >
                Coming to Flathub
              </Button>
            </div>
          </Card>
        </div>
      </div>
    </section>
  );
}
