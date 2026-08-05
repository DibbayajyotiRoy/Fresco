# Fresco — Terms of Use & Privacy

**Revision 2.** Last updated 2026-08-04. Applies to Fresco 1.1.37 and later.

> **Changed in revision 2:** declining the optional statistics now still sends
> the random install id, so that unique users are countable. Under revision 1 it
> did not. Every install is asked again because of this — your previous answer
> was given to a different question. See section 12.

Fresco is free software under the [GPL-3.0](LICENSE). There is no paid tier, no
account, no subscription, and nothing in the app is for sale. Nobody buys your
data, because there is no buyer and nothing to sell.

This document says exactly what leaves your computer, why, and how to stop it.
If anything here disagrees with what the app actually does, the app is wrong and
it is a bug: please [open an issue](https://github.com/DibbayajyotiRoy/fresco/issues).

---

## 1. The short version

Fresco asks you one question, once, the first time you open it:

**Either way, once a day, Fresco records that one install was active, in which
country, on which version.** That is the headcount: how many people use Fresco
and roughly where. It is treated as part of maintaining the app rather than as
an optional extra, because a release cannot be tested where its users are if
nobody knows where they are.

What the dialog actually asks about is the **detail**:

| You choose | What Fresco sends |
| --- | --- |
| **Accept all** | The headcount, plus your distro, desktop, session type, rendering backend, monitor count, install source, which features you use, error kinds, your city and region, and the exact time of each check-in. |
| **Decline optional** | The headcount only: a random install id, your country, the app version, and whether you installed the `.deb` or the Flatpak. Your check-in is stored as a **date**, not a time. |

Both answers can be changed at any time in **Settings → Share anonymous usage
statistics**. Section 6 explains how to send nothing at all, and section 8 covers messaging the maintainer, which is separate from all of this.

**Declining does not mean total silence, and this is the one thing on this page
worth reading twice.** It means the detail is not sent. You are still counted,
once a day, as one anonymous install in a country. If that is not acceptable to
you, section 6 turns everything off, and Fresco keeps working exactly the same.

## 2. Why any of this exists

Fresco runs on a moving target: X11 and Wayland, half a dozen compositors, and
distributions that ship different versions of mpv, libmpv, and GPU drivers. A
release that works perfectly here can be broken on Deepin, or on an older
libmpv, or on a compositor nobody on the project runs.

Without numbers, the only signal is a bug report from someone whose desktop is
already broken. With them, the ones people actually use get tested first.

That is the entire purpose. Concretely, the data is used to decide:

- which distributions and desktops get tested before a release,
- which compositor bugs are worth the work to fix,
- whether an old version still has enough users to keep supporting,
- which regions need a mirror, because downloading from GitHub is slow or
  blocked there.

It is **not** used to profile you, build an audience, target advertising, or
train a model, and it is not shared with, sold to, or made available to any
third party. There is no analytics vendor in the loop.

## 3. "Accept all" in full

Every field, with nothing omitted. Rows marked **always** are the headcount and
are sent under both answers; the rest is what "accept all" adds.

| Field | Example | Where it comes from |
| --- | --- | --- |
| Install id — **always** | `9f3c1e7a-…` | Random, generated on first run, stored in `install-id` next to your config. Never derived from hardware, MAC address, hostname, or username. |
| Country — **always** | `IN` | See section 5. |
| App version — **always** | `1.1.37` | Compiled in. |
| Channel — **always** | `deb` / `flatpak` | Detected at runtime from how Fresco is packaged. |
| Distro | `Ubuntu 24.04.1 LTS` | `PRETTY_NAME` from `/etc/os-release`. |
| Desktop | `COSMIC` | `XDG_CURRENT_DESKTOP`. |
| Session | `wayland` | `XDG_SESSION_TYPE`. |
| Backend | `wayland` / `x11` / `gnome-static` | Which renderer Fresco chose. |
| Hardware decode | `vaapi` / `nvdec` | What mpv negotiated. |
| Monitor count | `2` | How many outputs. Not their resolutions or arrangement. |
| Install source | `website` | The tag the install one-liner recorded (`website`, `github`, …). Absent for package-manager installs. |
| City — *accept all only* | `Kolkata` | See section 5. Never sent if you decline. |
| Region — *accept all only* | `WB` | See section 5. Never sent if you decline. |
| Feature counts | `wallpaper_set`, `add_from_link` | Which feature ran and its outcome. Never what you set, or from where. |
| Error kinds | `mpvpaper_spawn_failed` | The kind of failure and a message truncated to 500 characters. |

The install id identifies an *install*, not a person. Reinstalling produces a
new one. Two people sharing a computer share one. It exists so that one user
opening Fresco 40 times in a month counts as one user and not as 40.

**Sent at most once every 20 hours**, whether you open Fresco once that day or
fifty times.

## 4. "Decline optional" in full

One request a day, containing four things: the random install id, your country,
the app version, and whether you installed the `.deb` or the Flatpak.

It carries **no distro, no desktop, no session type, no rendering backend, no
monitor count, no install source, no feature usage, no error reports, and no IP
address.** Your check-in is stored with the **date truncated** — the server
writes `2026-08-04`, never `2026-08-04 21:47`. That is not a display choice:
`register_install_minimal` in [supabase/schema.sql](supabase/schema.sql)
truncates before the insert, so the time of day is never stored at all and
cannot be recovered.

What the install id does and does not do, plainly. It **does** mean your
check-ins on different days are recognisably the same install, which is the
entire point: it is what makes "412 people used Fresco this month" a real
number instead of a guess. It **does not** carry your name, your hardware, your
IP, or anything you did in the app under this tier — the row is an id, a
country, a version, and a set of dates.

If linking your days together is not something you want, section 6 turns it off
entirely.

## 5. Where you are, and how it is worked out

Fresco does not read your location, does not use GPS, does not use Wi-Fi
positioning, and does not store your IP address.

**Country** is sent under both answers. **City and region are optional**: they
are sent only if you accept all, and declining means they are never sent at
all — not sent-and-discarded.

### Country

Every network request necessarily reveals an IP to the server being contacted;
that is how the reply finds its way back. Fresco's requests are served by
Supabase, which sits behind Cloudflare. Cloudflare resolves the IP to a
two-letter country code **at the network edge** and forwards only that code.
The IP is never read, logged, or stored by Fresco or by its database — the
first thing this project ever sees is two letters. Because it is resolved
server-side, it also cannot be forged by a modified client.

### City and region

These need a lookup Cloudflare only performs on its Enterprise plan, which
Supabase does not have, so the database cannot see them. Instead the app asks
`https://fresco.dibbayajyoti.com/api/geo`, a route on Fresco's own website,
which reads the equivalent headers Vercel injects. That handler never reads,
logs, or returns an IP: Vercel resolves the address at the edge before it runs,
and the response contains place names only.

**Coordinates are deliberately not collected.** Vercel supplies latitude and
longitude on the same request and the handler drops them on the floor rather
than returning them, so there is nothing to store even by accident.

Because the app fetches these and then sends them, they are *client-supplied*
and could be faked by a modified build. Country cannot. Nothing in Fresco
depends on city being truthful; it decides which regions might want a download
mirror, and nothing else.

If you use a VPN, everything here reflects your VPN's exit, not you.


Fresco does not read your location, does not use GPS, does not use Wi-Fi
positioning, and does not send your IP address anywhere.

Every network request on the internet necessarily reveals an IP address to the
server being contacted; that is how the reply finds its way back. Fresco's
requests are served by Supabase, which sits behind Cloudflare. Cloudflare
resolves the IP to a two-letter country code **at the network edge** and
forwards only that code. The IP is never read, logged, or stored by Fresco or
by its database — the first thing this project ever sees is two letters.

The stored value is therefore `IN`, or `BR`, or `DE`. There is no city, no
region, no postcode, no network, no coordinates, and no way to derive any of
them later, because the input was discarded before it arrived.

If you use a VPN, the country recorded is your VPN's. That is fine; the number
is for deciding where to put a download mirror, not for knowing where you are.

## 6. Turning it off completely

The Settings switch chooses between the two tiers in section 1. To send
**nothing at all**, edit `~/.config/fresco/config.toml`:

```toml
telemetry = false
telemetry_prompted = false
```

With both set, Fresco is silent: no heartbeat, no country ping, no events, no
errors. The consent dialog will ask again next launch; set them again, or
answer it.

You can also confirm any of this yourself rather than take this page's word for
it. Fresco is GPL-3.0 and every line that sends anything is in
[`src/telemetry.rs`](src/telemetry.rs) — one file, and every network call in it
goes through a single function.

## 7. What is never collected

Under any setting, ever:

- Your name, email, or any account (Fresco has no accounts).
- Your IP address, and your coordinates (see section 5).
- Your city, if you declined the optional statistics.
- Your wallpapers, their file names, their contents, or their paths.
- Any file path at all, including the folders you browse.
- Screenshots or screen contents.
- Audio. The visualiser analyses your system audio **on your machine only**, and
  asks separately before it may listen at all. Not one byte of audio, and no
  derived value from it, is ever transmitted.
- Song titles, artists, or what you listen to. Lyrics are fetched by song name
  from LRCLIB, which is a separate service with its own privacy policy, and only
  when you enable the lyrics widget.
- Keystrokes and clipboard contents. Nothing you type is captured anywhere. The
  one exception is text you deliberately write and press send on: a feedback
  comment, or a message to the maintainer (section 8) — and that is not
  collection, it is you sending a message.

## 8. Messaging the maintainer

**Menu → Message the maintainer** opens a private, two-way thread with the
person who makes Fresco. It exists so a problem can actually be worked through,
instead of ending at a one-line rating nobody can follow up on.

It is anonymous in **both** directions. You never learn who they are beyond
"the maintainer", and they never learn who you are. What they see is:

- the messages you type,
- the setup summary shown in the dialog (app version, distro, desktop, session,
  backend), which you can untick before sending,
- a random ticket id.

That ticket is generated separately from everything in section 3 and stored in a
different file. It is **not** your telemetry install id, and the two are never
sent in the same request. This is deliberate: a support thread is the one place
you write in your own words, and keying it by the telemetry id would make it
possible to attach that writing to an environment profile. It is also why
support works normally if you declined the optional statistics — the two systems
do not know about each other.

Nothing is created until you send a first message. If you never write, no thread
exists.

Holding the ticket is what authorises reading the thread, so keep in mind that
anyone with access to your user account can open Fresco and read the
conversation, exactly as they could read your email client.

To delete a thread, say so in it and it will be removed, or delete
`~/.local/state/fresco/support-ticket` to abandon it — a new message after that
starts a fresh thread with no link to the old one.

## 9. Other network connections

Separate from statistics, and listed so this page is complete:

| What | When | Sends |
| --- | --- | --- |
| Update check | Periodically | Nothing but the request itself. |
| Wallpaper catalog | When you open Browse wallpapers | Nothing but the request itself. |
| Add from link | When you paste a URL | The URL you pasted, to the site you pasted it from. |
| Lyrics (LRCLIB) | Only with the lyrics widget on | The track title and artist your media player reports. |
| Feedback | Only when you submit it | Your rating and comment, your timezone and locale. Anonymous, and sent only on the button press. |
| Support thread | Only after you send a first message | See section 8. Checked for replies every 30 minutes while the daemon runs, and only once a thread exists. |

## 10. Retention, and asking for deletion

Counts are kept indefinitely; they are what makes "is this distro still worth
supporting" answerable across years. Nothing in them is personal data as of the
day it is stored.

If you accepted all and want your install's row deleted, open an issue with your
install id (the contents of `~/.config/fresco/install-id`) and it will be
removed. Country-only counts (section 4) cannot be deleted on request — not as a
refusal, but because there is genuinely no row to find: your ping was added to a
shared integer and is no longer distinguishable from anyone else's.

## 11. Terms of use

Fresco is licensed under the [GPL-3.0](LICENSE). You may use, study, modify, and
redistribute it under those terms. In short:

- **No warranty.** Fresco is provided as-is. It paints your desktop background;
  it is not fit for any safety-critical purpose, and the authors are not liable
  for any loss arising from its use, to the extent the law allows.
- **Your content is yours.** Fresco claims no rights over the videos, GIFs, or
  images you set as wallpapers. They stay on your machine.
- **The built-in catalog** contains works under their own licenses, shown on
  every item. Respect them.
- **Add-from-link** fetches whatever URL you give it. Making sure you are
  allowed to use that media is your responsibility, not Fresco's.
- **You must not** use Fresco to distribute unlawful material, or represent a
  modified build as official.

## 12. Changes to these terms

If what Fresco collects changes, this revision number goes up and **the app asks
you again**. Your previous answer is not carried over onto different terms — the
consent dialog stores which revision you agreed to, and re-asks when it no
longer matches. Revision 1 is the first in which declining still sends a country
count; installs that answered under revision 0 are asked once more, because
declining meant something different then.

### Revision history

| Rev | Change | Re-asked? |
| --- | --- | --- |
| 1 | Declining stopped meaning total silence: it sent an identifier-free country tally. | Yes |
| 2 | Declining now also sends the random install id, so unique users are countable in both tiers. In exchange, the essential tier's check-in time is truncated to a date. Accepting all additionally sends city and region. | Yes |

Contact: <https://github.com/DibbayajyotiRoy/fresco/issues>
