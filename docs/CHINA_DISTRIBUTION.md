# Distributing Fresco in mainland China

**Status:** plan, not yet executed. Nothing in here has been set up.
**Last researched:** 2026-07-31.

---

## 1. Why this matters

A Deepin community tester in mainland China reviewed Fresco and named distribution
as the single highest-priority gap before release:

> "Inconvenient distribution: currently GitHub Releases only, which is unstable to
> reach from mainland China. Consider listing on the official deepin app store
> (https://appdelivery.deepin.org.cn/#/index), and/or publishing mirrors on Gitee /
> GitCode. I'd treat this as the top priority before release."

Everything Fresco ships today funnels through `github.com` / `api.github.com`:

| Path | Endpoint | Source |
|---|---|---|
| One-liner install | `https://github.com/DibbayajyotiRoy/fresco/releases/latest/download/install.sh` | `install.sh:3` |
| Installer's package lookup | `https://api.github.com/repos/DibbayajyotiRoy/fresco/releases/latest` | `install.sh:49` |
| In-app update check | `https://api.github.com/repos/DibbayajyotiRoy/fresco/releases/latest` | `src/update.rs:20` |
| Auto-updater (run via pkexec) | same API | `scripts/fresco-update.sh:17` |
| "Releases" links in GUI / notifier | `https://github.com/.../releases/latest` | `src/gui/updates.rs:269`, `src/daemon/notifier.rs:40` |

So for a mainland user, *installing* and *updating* both fail or stall on the same
choke point. Note that `scripts/fresco-update.sh` already reads
`REPO="${FRESCO_REPO:-DibbayajyotiRoy/fresco}"` — it is parameterised by repo but
**not** by host, so it cannot currently be pointed at a mirror.

**Who the audience is.** Deepin is the open-source upstream community edition of
UnionTech's UOS; UOS itself targets Chinese government/enterprise, while Deepin is
mostly community users — developers, DIY builders, Linux early adopters. That is
exactly Fresco's existing user profile, and it is a segment that will happily run a
one-line installer *if the bytes arrive*. Deepin 25's default session is X11
(Treeland, `linuxdeepin/treeland`, is not yet daily-usable), which is Fresco's
best-supported backend — the `dde` leg of `_environments.yml` already covers it.

A competing app, **Spark Live Wallpaper (星火动态壁纸)**, already ships Chinese
localization and is listed in the deepin app store. Being absent from every channel
a Deepin user actually browses is a bigger gap than any missing feature.

---

## 2. What exists today (release machinery)

Read before proposing changes — this plan bolts onto the following, it does not
replace it. See [`CI_AND_RELEASE.md`](CI_AND_RELEASE.md) for the full picture.

| Workflow | Trigger | Relevance here |
|---|---|---|
| `.github/workflows/publish.yml` | push to `main` | Jobs: `checks` → `environments` → `version` → `release`. The `release` job calls `_release.yml` **only** when `Cargo.toml`'s version has no matching tag. |
| `.github/workflows/release.yml` | push a `v*` tag | Single job `release`, also calls `_release.yml`. |
| `.github/workflows/_release.yml` | `workflow_call` | Jobs: `build-mpvpaper-libmpv2` → `build-deb`. The `build-deb` job's step **"Upload artifacts to release"** (`softprops/action-gh-release@v2`) is where `target/debian/fresco_*.deb` and `install.sh` get attached to the GitHub release. |
| `.github/workflows/distros.yml` | weekly + manual | Cross-distro build/install matrix. |
| `.github/workflows/_ci.yml`, `_environments.yml` | `workflow_call` | Strict checks; headless DE matrix with a "≥3 must pass" gate. |

Convention: `_`-prefixed workflows are reusable (`workflow_call`) and never run on
their own.

**Artifact shape.** `cargo deb` with `[package.metadata.deb]` in `Cargo.toml`
produces a single `amd64` .deb bundling two mpvpaper builds
(`/usr/lib/fresco/mpvpaper-libmpv1` and `-libmpv2`, one per libmpv soname
generation). Latest release `v1.1.36`:

```
fresco_1.1.36-1_amd64.deb   3,533,568 bytes  (3.4 MB)
install.sh                      4,862 bytes
```

3.4 MB per release is small, and that materially changes the calculus below —
Gitee's free attachment quota is not a real constraint for years.

**Architecture.** amd64 only today. Relevant because UOS/Deepin also target
domestic chips — x86 (Zhaoxin, Hygon), ARM (Kunpeng, Phytium, Kylin), LoongArch
(Loongson). See §8.

---

## 3. Channel comparison

| Channel | Effort | What it unblocks | Blockers | Automatable from CI? |
|---|---|---|---|---|
| **GitHub Releases** (today) | — | Everything outside CN | Unreliable from mainland CN | Already automated (`_release.yml`) |
| **Gitee release mirror** | Low (½ day + account) | Fast `.deb` download + a working `install.sh`/update path in CN | Account likely needs a **mainland phone number**; new public repos have historically gone through **manual open-source review (开源审核)** | **Yes** — Gitee OpenAPI v5 has documented create-release + upload-attachment endpoints (verified, §5) |
| **GitCode mirror** (CSDN + Huawei Cloud) | Low–medium | Second CN-reachable download host; CSDN-community discoverability | CSDN account (registration requirements unverified); reputational baggage from mass unconsented GitHub mirroring | **Partly** — public OpenAPI documents *read* and *update* release endpoints but **not** create-release or asset upload; treat as manual until verified |
| **Spark Store (星火应用商店)** | Low–medium | The store Deepin community users actually browse; the direct competitor is there | Submission is done from inside the store client (needs a Deepin/Debian box); `lintian` package-name check must pass | No — human submission |
| **deepin app store** (`appdelivery.deepin.org.cn`) | High | Official first-party listing, maximum trust | **Developer identity verification (开发者认证)**; docs disagree on deb-only vs deb+linglong; compat matrix is deepin v20/v23 x86 | No — human submission + review |
| Landing-page CN mirror/CDN | Medium | Faster site, not faster downloads | ICP filing (备案) *only if* hosted in mainland (§7) | N/A |

---

## 4. Recommended sequencing

> ### ⚠️ Superseded 2026-08-04 — Gitee is no longer step 1
>
> This section originally opened with the Gitee mirror. Two findings overturned that:
>
> 1. **Gitee is blocked for this maintainer.** Account registration from India
>    succeeded, but binding a mobile number — required to use *import from GitHub* /
>    repo mirroring — **rejected a +91 number**. Gitee publishes no international-number
>    route: its help centre has no page on 实名认证, phone binding, country codes or
>    access tokens at all (verified by grepping all 735 URLs in `help.gitee.com/sitemap.xml`).
>    Owning the mirror personally is not available.
> 2. **Spark Store needs no account at all**, and hosts the package itself — so it
>    solves both the identity problem and the hosting problem in one step. See §8a.
>
> **Revised order: Spark Store first (§8a). Gitee becomes an optional later mirror
> that a mainland collaborator would have to own.** Do not treat a CN-reachable source
> repo as a prerequisite for the store listing — Spark Store does not require one.
>
> The `install.sh` and update-path changes were still worth landing and are done:
> they cost little and make any future mirror a two-line change.

1. ~~**Gitee release mirror + `FRESCO_SOURCE=gitee` in `install.sh`.**~~
   **Blocked — see the box above.** Retained for the record: it would fix both
   *install* and *update* for mainland users and is fully automatable, but it
   requires a mainland-verified account to set up.

2. **Spark Store submission.** This is the channel that actually reaches the
   tester's peer group, and it is the one the competitor already occupies. It is
   a community store with a lightweight, email-keyed submission flow — far less
   ceremony than the official store, and it accepts a plain `.deb`. Do it as soon
   as you have a CN-reachable download URL from step 1.

3. **deepin official app store.** Do this third, not first. It carries the most
   trust but also the only hard identity-verification gate, an unclear
   deb-vs-linglong story, and a compatibility matrix (deepin v20/v23) you must
   test against. It is worth doing — just not worth blocking the release on.

4. **GitCode mirror.** Optional redundancy. Cheap to add once Gitee works
   (same `.deb`, one more upload), but it duplicates step 1's benefit rather than
   adding a new audience. Skip it unless Gitee proves unreliable or the review
   gate blocks you.

5. **Chinese localization (zh-CN).** Out of scope for this doc, but flagged: the
   competitor ships it, and both stores' review flows judge on presentation. Being
   in the store in English is still much better than not being in the store.

**Explicitly deprioritised:** hosting anything (site, CDN, download endpoint)
*inside* mainland China. That is the only item on the board that would drag in an
ICP filing — see §7.

---

## 5. Step 1 — Gitee mirror (detailed)

### 5a. Manual setup (maintainer, one-time)

> ⚠️ **You are outside China.** Steps marked 🇨🇳 are the ones that may require a
> mainland phone number or ID. Read §9 before spending time.

1. **Create a Gitee account** at <https://gitee.com>. 🇨🇳
   Gitee verifies with a phone number like essentially all mainland platforms, and
   reports from overseas users are that some features (e.g. public gists) require
   a **mainland** number specifically
   ([Gitee 帮助中心 — 帐号与安全](https://help.gitee.com/account-and-profile)).
   Whether plain account creation + a public repo + releases can be done with a
   non-CN number **is not confirmed** — see §9. If it cannot, the fallback is to
   have a trusted mainland contributor own the mirror org and add you as a member.

2. **Create the mirror repo** `fresco` under your account/org, and set it up as a
   **Pull-direction mirror** from GitHub:
   Repo → 管理 → **仓库镜像管理** → **添加镜像**, choose Pull direction, bind the
   GitHub account, and supply a GitHub personal access token with `repo` scope
   (plus `admin:repo_hook` if you want the automatic mode rather than manual)
   ([Gitee 帮助 — 仓库镜像管理](https://help.gitee.com/repository/settings/sync-between-gitee-github)).
   Minimum 5 minutes between syncs; a sync over 30 minutes times out.

   **Important:** the mirror syncs **branches, tags and commits only** — the same
   doc does not list releases or release attachments. That is exactly why step 5b
   exists: the `.deb` must be pushed to Gitee separately.

3. **Wait out the open-source review (开源审核).** 🇨🇳
   Since May 2022 Gitee has required newly-published open-source repos to pass a
   manual human review before going public; repos are held private until they pass
   ([IT之家](https://www.ithome.com/0/619/320.htm),
   [InfoQ](https://www.infoq.cn/article/2ttlpz58wfskk852f04d)).
   Whether this is still enforced in 2026 is **not confirmed** (§9) — but plan for
   it. Fresco is GPL-3.0-or-later with a clean provenance story, so it should pass;
   have the licence, the repo description and a Chinese-language `README.zh-CN.md`
   ready to shorten the round trip.

4. **Generate a Gitee private access token** (设置 → 私人令牌) with repo scope, and
   add it to the GitHub repo as the secret **`GITEE_TOKEN`**. Also add
   **`GITEE_REPO`** (e.g. `dibbayajyoti/fresco`) as a variable or secret so the
   workflow does not hard-code it.

### 5b. Quotas — verified, and they are fine

From [Gitee 产品配额说明](https://help.gitee.com/questions/Gitee%E4%BA%A7%E5%93%81%E9%85%8D%E9%A2%9D%E8%AF%B4%E6%98%8E)
(free/personal tier):

| Limit | Value | Fresco's number |
|---|---|---|
| Git repo size | 500 MB | Source repo is well under |
| Single file in git | 50 MB | n/a |
| **Release attachment, single file** | **100 MB** | 3.4 MB ✅ |
| **Total attachments per repo** | **1 GB** | ~290 releases' worth ✅ |

At Fresco's current cadence (1.1.34 → 1.1.36 in six days) 1 GB is still roughly two
years. Add a housekeeping note: prune Gitee attachments older than the last ~20
releases once you pass ~600 MB.

### 5c. The Gitee release API (verified against Gitee's own swagger)

Fetched from `https://gitee.com/api/v5/swagger_doc.json` on 2026-07-31:

```
POST   /v5/repos/{owner}/{repo}/releases
       formData: access_token, tag_name*, name*, body*, target_commitish*, prerelease
GET    /v5/repos/{owner}/{repo}/releases/tags/{tag}       query: access_token
POST   /v5/repos/{owner}/{repo}/releases/{release_id}/attach_files
       formData: access_token, file*        (multipart; the field name is literally `file`)
GET    /v5/repos/{owner}/{repo}/releases/latest           query: access_token
```

(`*` = required. Base: `https://gitee.com/api/v5`.
[Gitee API 文档](https://gitee.com/api/v5/swagger).)

`target_commitish` is **required** on create. If the tag already exists on Gitee
(it will, once the mirror has synced) Gitee uses it; otherwise it creates the tag at
`target_commitish`.

**Live-verified response shape** (probed against the public
`spark-store-project/spark-store` repo):

```json
{"id":751458,"tag_name":"5.2.1.0","assets":[
  {"browser_download_url":"https://gitee.com/…/releases/download/5.2.1.0/spark-store_5.2.1.0_amd64.deb",
   "name":"spark-store_5.2.1.0_amd64.deb"}, …]}
```

The JSON key is `browser_download_url` — **identical to GitHub's**. That is what
makes §6 cheap. One gotcha, also verified: **Gitee returns minified JSON** (zero
newlines, and no space after the colon: `"browser_download_url":"https://…"`),
whereas GitHub pretty-prints. `install.sh`'s current line-oriented parser therefore
does *not* work verbatim against Gitee — see §6.

### 5d. Wiring it into CI

A draft workflow has been added at
**`.github/workflows/draft-gitee-mirror.yml`**. It is `workflow_dispatch`-only, so
it cannot fire on a push or a tag, and it no-ops loudly if `GITEE_TOKEN` is unset.
It downloads the assets already attached to a GitHub release (via `gh release
download`) and re-uploads them to the matching Gitee release. Run it by hand for one
release first.

Once it has worked by hand twice, promote it by adding a job to
**`publish.yml`** (and, if you want tag-pushes mirrored too, to `release.yml`)
that runs *after* `release`:

```yaml
  # publish.yml — add after the existing `release` job
  mirror-cn:
    needs: [version, release]
    if: needs.version.outputs.should_publish == 'true'
    uses: ./.github/workflows/draft-gitee-mirror.yml   # rename to _gitee-mirror.yml when promoting
    with:
      tag: ${{ needs.version.outputs.tag }}
    secrets: inherit
```

To do that you must also convert `draft-gitee-mirror.yml` to a reusable
(`workflow_call`) workflow and rename it `_gitee-mirror.yml` to match this repo's
`_`-prefix convention. **Do not** put the Gitee upload inside `_release.yml`'s
`build-deb` job: that job's contract is "publish the installable artifacts first so
a flaky smoke test can never block the release", and a Gitee outage must not be able
to fail a GitHub release. A separate downstream job keeps that property.

---

## 6. Step 1b — teaching `install.sh` about mirrors

`install.sh` already has the hook for this. It reads a source tag purely for
telemetry attribution:

```sh
# Download-source attribution (UTM-style): the copy buttons on the website /
# README / posts prefix the one-liner with FRESCO_SOURCE=<tag>. Persisted for
# the app's anonymous telemetry (reported only if the user opts in). No tag =
# "installer".
FRESCO_SOURCE="${FRESCO_SOURCE:-installer}"
mkdir -p "$HOME/.config/fresco" 2>/dev/null || true
printf '%s' "$FRESCO_SOURCE" > "$HOME/.config/fresco/install-source" 2>/dev/null || true
```

and the download itself is hard-wired to GitHub:

```sh
REPO="DibbayajyotiRoy/fresco"
…
# 3. Fetch latest .deb URL from GitHub Releases API
info "Fetching latest release from GitHub…"
API_URL="https://api.github.com/repos/${REPO}/releases/latest"
DEB_URL=$(curl -fsSL "$API_URL" \
  | grep '"browser_download_url"' \
  | grep '\.deb"' \
  | head -1 \
  | sed 's/.*"browser_download_url": "\(.*\)".*/\1/')
```

**The minimal change** — keep `FRESCO_SOURCE` doing double duty as the attribution
tag *and* the mirror selector, so the existing copy-button plumbing and telemetry
keep working unchanged:

```sh
# Mirror selection. FRESCO_SOURCE also picks the download host, so a mainland
# user can run:  FRESCO_SOURCE=gitee bash -c "$(curl -fsSL https://gitee.com/<owner>/fresco/releases/download/latest/install.sh)"
GITEE_REPO="${FRESCO_GITEE_REPO:-<owner>/fresco}"
case "$FRESCO_SOURCE" in
  gitee)   API_URL="https://gitee.com/api/v5/repos/${GITEE_REPO}/releases/latest"; HOST="Gitee" ;;
  *)       API_URL="https://api.github.com/repos/${REPO}/releases/latest";         HOST="GitHub" ;;
esac
info "Fetching latest release from ${HOST}…"

# NB: Gitee returns MINIFIED json ("key":"value", no space, one line), GitHub
# pretty-prints ("key": "value"). Parse with grep -o so both work.
DEB_URL=$(curl -fsSL "$API_URL" \
  | grep -o '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]*\.deb"' \
  | head -1 \
  | sed 's/.*"\(https[^"]*\.deb\)"/\1/')
```

That `grep -o` form is a strict improvement even on GitHub — the current
`sed 's/.*: "\(.*\)".*/\1/'` is greedy and only survives because GitHub happens to
put one URL per line.

**Also needs the same treatment** (do these in the same change, or Chinese users
install fine and then never get updates):

- `scripts/fresco-update.sh:16-17` — parameterise the host, not just
  `FRESCO_REPO`. It should read the persisted `~/.config/fresco/install-source`
  and use Gitee when that says `gitee`.
- `src/update.rs:20` (`RELEASES_API`) — same: pick the API base from the persisted
  install source.
- `src/gui/updates.rs:268-269` and `src/daemon/notifier.rs:40` — the human-facing
  "Releases" URL and the copy-paste install command shown in the GUI should point
  at Gitee for those users.

> Those files are owned by other work in flight right now — this doc deliberately
> does not touch them. Treat the snippets above as the spec.

**Where the mirrored `install.sh` lives.** `_release.yml` already attaches
`install.sh` to every GitHub release, and the draft mirror workflow re-uploads
whatever the GitHub release holds — so the Gitee release gets `install.sh` for free.
The mainland one-liner then becomes:

```sh
FRESCO_SOURCE=gitee bash -c "$(curl -fsSL https://gitee.com/<owner>/fresco/releases/download/v1.1.36/install.sh)"
```

⚠️ **Needs verification:** whether Gitee exposes a stable `…/releases/download/latest/<file>`
alias the way GitHub does. The live probe only confirms per-tag URLs
(`…/releases/download/5.2.1.0/…`). If there is no `latest` alias, publish the
mainland one-liner against the raw file in the repo
(`https://gitee.com/<owner>/fresco/raw/main/install.sh`) instead, which the mirror
keeps current anyway.

---

## 7. ICP filing (备案) — what actually applies

**Short answer: nothing, as long as nothing moves onto mainland servers.**

ICP filing with MIIT is required when a domain resolves to a server **inside**
mainland China and a web service is served from it. Sites hosted outside the
mainland do not need MIIT ICP filing
([Alibaba Cloud — ICP filing FAQ](https://www.alibabacloud.com/help/en/icp-filing/faq-about-icp-filing-applications-in-different-scenarios/),
[Alibaba Cloud — ICP filing for overseas enterprises](https://www.alibabacloud.com/help/en/icp-filing/basic-icp-service/product-overview/icp-filing-application-for-enterprises-outside-the-chinese-mainland),
[chinafy 2025 guide](https://www.chinafy.com/blog/a-2025-guide-to-icp-licences-in-china-do-i-need-an-icp-license-for-my-website)).

`fresco.dibbayajyoti.com` is hosted outside China, so it needs no filing.

Practical implications:

- **Gitee / GitCode / the app stores carry their own filings.** Hosting the `.deb`
  there is precisely how you get CN-side delivery *without* taking on any filing
  obligation yourself. This is the whole argument for step 1.
- **Do not** put a mainland CDN in front of `fresco.dibbayajyoti.com`, and do not
  stand up a `download.fresco…` endpoint on a mainland host. Either would trigger
  the filing requirement (and ICP filing generally requires a mainland business
  entity, which you do not have).
- The landing page itself may still be slow from China. That is an inconvenience,
  not a blocker, once the store listings and the Gitee release page exist — those
  become the canonical CN entry points. Link them prominently from the site and
  README.
- ⚠️ Note that overseas-hosted sites are sometimes described as still needing a PSB
  (公安备案) filing. This is **not confirmed** as applying to a foreign-hosted
  personal open-source project site, and no action is proposed here — flagged only
  so it is not a surprise (§9).

---

## 8. Step 2 & 3 — store listings

### 8a. Spark Store (星火应用商店) — **do this FIRST**

A community-run app store built by the 星火工作组 out of the Deepin BBS, targeting
deepin/Debian-family distros across amd64/arm64/loongarch64. Official site
<https://www.spark-app.store/>; source at
<https://gitee.com/spark-store-project/spark-store>. It is what Deepin community
users actually browse, and it is where the competitor sits.

**Why it is now step 1: it needs no account, and it hosts the package for you.**
Verified against the store's own submission guide (self-dated 2025-06-14,
<https://wiki.spark-app.store/Submit/Submit.md>), the *entire* identity requirement
is a free-text packager string:

> 在下图填写昵称和邮箱，格式为 `昵称<邮箱>` … 邮箱会被用于审核志愿者与您联系

No account, no phone binding, no 实名认证, no Gitee login, no QQ appears anywhere in
the amd64 path. And acceptance removes the hosting problem entirely: Spark Store runs
its own tiered CDNs (`sucdn.jerrywang.top` / `store.jerrywang.top` mainland,
`cfcdn.jerrry.wang` Cloudflare, `dcstore.spark-app.store` direct) plus a donated
Shandong University rsync mirror
(<https://wiki.spark-app.store/Distribution/README.md>). You never need China-side
storage of your own.

**Two submission mechanisms:**

1. **Web submitter — no install needed:** <https://upload.deepinos.org.cn/index>
   (confirmed HTTP 200 from outside China, `<title>投稿系统</title>`). Their guide
   notes *"您需要使用 Linux 来打开此网页"*, and that it does not auto-fill app info
   and cannot submit domestic architectures. **This is the entry point when you have
   no Deepin VM.**
2. **Desktop submitter** (`spark-store-submitter`, installed from the store itself
   via `spk://store/tools/spark-store-submitter`) — drag the `.deb` in and package
   name, version, homepage, author and description auto-populate from the control
   file. Better, but chicken-and-egg: you need Spark Store installed first.

**What you must supply:**

- `.deb`, **amd64**. Package name rules: lowercase letters/digits/`+`/`-`/`.`, ≥2
  chars, must start with a letter, ≤30 chars, must not collide with a distro package
  name. Enforced automatically with `lintian`; **`bad-package-name` is an automatic
  reject** (<https://wiki.spark-app.store/Submit/DEB-SPEC.md>). Fresco's name is
  `fresco` — valid, but run `lintian` on the built artifact rather than assuming.
- **Icon:** PNG, ≥128×128 and ≤512×512. `data/icons/hicolor/256x256/` fits.
- **Screenshots:** PNG, at least one, at most five.
- **Description in Chinese** — *"如无特殊情况，应用详情应当是中文"*, and no emoji or
  unusual symbols. The zh_CN text now in
  `data/io.github.dibbayajyotiroy.Fresco.metainfo.xml` is the obvious source.
- **Tags** — apps requiring system services must tick **必须安装到主机**. Fresco ships
  a daemon (`frescod`) and writes an autostart entry, so this applies. Getting it
  wrong is a common rejection cause.
- Fresco is upstream-official, not a repack, so it does **not** need the `.spark`
  package-name suffix repackagers must use.
- Optional 社区开发 tag for open-source apps (a badge on the detail page), explicitly
  not mandatory. GPL-3.0 is squarely fine: *"对于开源软件，请遵循其开源协议"*.

Keep the **contact email and app name stable** — updates are matched on that pair;
change either and it is treated as a brand-new app, orphaning existing installs.

**Review:** volunteers install with `sudo ssaudit <path>`, check the `.desktop` file
launches, check clean uninstall, and inspect maintainer scripts for dangerous
commands (<https://wiki.spark-app.store/Manage/Audit.md>). Fresco's
`packaging/debian` postinst is therefore worth re-reading before submitting. On
approval you get an email and the package is pushed at **01:30 GMT+8 daily**; on
rejection you get an email with the reason. Questions: `shenmo@spark-app.store`.

**Two cautions from the primary source:**

- **Content must not violate PRC law.** For a wallpaper app that fetches remote
  media, think about which sources ship enabled by default. **Pinterest is not
  reachable from the mainland** — a default that dead-ends is both a review risk and
  a poor first-run experience. Consider what the zh-CN build defaults to.
- **arm64 / loong64 submission is QQ-gated:** you must click 打包一下 instead of
  投稿一下, then send the resulting `tar.gz` to **QQ group 754330902** and @ the group
  owner. Whether a foreign individual can practically obtain a QQ account is
  unconfirmed. **Ship amd64 first** — it is the overwhelming majority of desktop
  users, and Fresco is amd64-only today anyway.

### 8b. deepin official app store — do this third

Portal: <https://appdelivery.deepin.org.cn/>. Register as a deepin community user
first, then use the delivery system
([deepin.org — Instructions for Using the Community Delivery System](https://www.deepin.org/en/deliver-applications/),
[DeepinWiki](https://wiki.deepin.org/en/About_Deepin/app_delivery)).

What the official docs say:

- **Package format — the two official pages disagree, and this matters.**
  The English delivery instructions say: *"The app store currently only support deb
  format application packages. Please ensure that the application package you upload
  is in the .deb format."*
  ([deepin.org/en/deliver-applications](https://www.deepin.org/en/deliver-applications/)).
  The Chinese listing guide says the supported formats are **"DEB / 玲珑格式"**
  ([deepin.org/zh — 应用商店上架指南](https://www.deepin.org/zh/app-store-submission-listing-guide/)).
  A third community summary claims deb/rpm/source. **Plan for deb**, which Fresco
  already produces; treat linglong as an upgrade path, not a gate.
- **Developer identity verification is required.** 🇨🇳 The delivery instructions
  state developer identity verification is required, with valid contact information
  and official website / source repository documentation. The Chinese guide says
  individual developers are explicitly supported — *"完全可以，完成个人开发者认证，
  即可正常提交上架"* — and that you submit **either** individual **or** company
  credentials ("个人或企业资质资料，选择其一即可"). Neither page states whether a
  Chinese national ID is mandatory. **This is the single biggest unknown in the
  plan** (§9).
- **Compatibility matrix.** The English page states current system compatibility
  covers **X86 architecture on deepin v20 and v23 only**. Fresco is amd64-only, so
  that is aligned — but you must actually test the `.deb` on deepin 20 and 23.
  Deepin 20 is Debian 10-era and ships **GTK4 4.6 at best or not at all**; Fresco's
  `depends` line requires `libgtk-4-1 (>= 4.6), libadwaita-1-0 (>= 1.1)`. ⚠️ Expect
  deepin 20 to fail dependency resolution — verify before submitting, and if it
  does, submit for deepin 23 only rather than shipping a package that cannot install.
- **Listing assets you will need to prepare:** app name ≤60 chars, category, a
  one-sentence description ≤100 chars, region selection, system compatibility, full
  description ≤1000 chars, changelog ≤1000 chars per update, icon 96×96–512×512
  (JPG/PNG), and **3–6 screenshots** (JPG/PNG, ≤2 MB each). Fresco already has
  suitable icons under `data/icons/` and AppStream metadata in
  `data/io.github.dibbayajyotiroy.Fresco.metainfo.xml` to crib the descriptions from.
- **Review** covers package-format detection, security detection, and compliance
  (documentation, compatibility, and a final application review). Failures come back
  with 整改意见 (remediation notes) and you resubmit. No published SLA.

**If linglong becomes mandatory.** Deepin has been pushing linglong (玲珑, upstream
project "linyaps") as its package format. You would not hand-write it: `linglong-pica`
converts a `.deb` into a linglong package and generates the `linglong.yaml`
([linyaps docs](https://linyaps.org.cn/en/guide/ll-builder/linyaps_package_spec.html),
[deepin.org — Linglong 10-minute quick build guide](https://www.deepin.org/en/linglong-10-minute-quick-build-guide/)).
Tooling: `sudo apt install linglong-builder linglong-box linglong-bin`, then
`ll-builder build`. ⚠️ **Big caveat:** linglong is a *sandboxed* container format
(`ll-box`), so it will hit the exact same class of problems documented in
[`FLATHUB.md`](FLATHUB.md) — wlr-layer-shell access, a daemon that must outlive the
GUI, `--filesystem=host:ro`-equivalent access to arbitrary user media, and writing an
autostart entry. Do **not** treat linglong as a mechanical repackage; budget it as
its own project, and only after Flatpak's sandbox story is settled.

### 8c. GitCode — optional, step 4

GitCode (<https://gitcode.com>) is operated by 重庆开源共创科技有限公司, launched
2023-09-22 by **CSDN together with Huawei Cloud CodeArts**
([GitCode 帮助文档 — 关于我们](https://docs.gitcode.com/v1-docs/docs/aboutus/)).

- **Releases with attachments are supported** via the web UI: project → Code tab →
  Tags → Release → **+New Release**, with file attachments for download
  ([GitCode 帮助文档 — 发行版](https://docs.gitcode.com/v1-docs/docs/repo/code/release/)).
- **CI automation is not currently viable.** The public OpenAPI reference
  (`https://api.gitcode.com/api/v5/repos/{owner}/{repo}/releases`, token passed as
  an `?access_token=` query parameter) documents **list / get-by-tag / update** but
  **not** create-release or asset upload
  ([GitCode 帮助文档 — release 接口文档](https://docs.gitcode.com/v1-docs/docs/openapi/repos/release/)).
  Note the path shape is `/api/v5/...`, deliberately Gitee-compatible, so an
  undocumented create endpoint may well exist — ⚠️ needs verification (§9).
- **Import from GitHub** is supported (import-by-URL / import-from-GitHub).
- ⚠️ **Reputational note worth knowing before you associate the project with it:**
  GitCode mass-mirrored large numbers of GitHub repositories *without author
  consent*, including mirroring user accounts and rewriting GitHub links in READMEs
  to point at GitCode
  ([discussion](https://www.zhihu.com/question/659859887)). Creating your *own*
  mirror deliberately is unaffected by this, but it is a reason to rank GitCode
  below Gitee rather than beside it — and a reason to check whether an unconsented
  `gh_mirrors/`-namespace copy of Fresco already exists and, if so, claim or remove it.

---

## 9. Open questions / needs verification

Ordered by how much they can derail the plan.

1. **Can a maintainer outside China register a Gitee account and publish public
   releases without a mainland phone number?** 🇨🇳 Phone verification is standard on
   Gitee; overseas reports indicate some features specifically require a mainland
   number. Not confirmed for the account + public repo + release path.
   *If blocked:* have a trusted mainland collaborator own a Gitee **organisation**
   and add you as a member, or fall back to GitCode. **Verify this first — the whole
   sequencing depends on it.**
2. **Is Gitee's manual open-source review (开源审核) still enforced in 2026, and how
   long does it take?** Confirmed as policy from May 2022; no current source found.
   Affects how early you must start.
3. **Does the deepin app store's developer verification (开发者认证) require a
   Chinese national ID?** The docs say individual developers are supported and that
   individual *or* company credentials suffice, but do not say what documents are
   accepted from a foreign individual. This is the gate on step 3.
4. **deb or linglong at the deepin store?** The English and Chinese official pages
   disagree (deb-only vs DEB/玲珑). Ask in the delivery portal or on
   <https://bbs.deepin.org> before packaging anything.
5. **Does Fresco's `.deb` install on deepin 20?** Its GTK4/libadwaita `depends` are
   very likely too new. Test on deepin 20 and 23 VMs before submitting.
6. **Does GitCode have an undocumented create-release / upload-asset API?** Its
   `/api/v5/` paths mirror Gitee's, so it plausibly does. Determines whether step 4
   can be automated or stays manual.
7. **Does Gitee expose a `releases/download/latest/<file>` alias?** Only per-tag
   URLs were observed. Affects the exact mainland one-liner in §6.
8. **Are Spark Store submission and the deepin delivery portal usable from outside
   China** (network reachability, captcha/SMS gates)? Unknown.
9. **PSB (公安备案) for a foreign-hosted site.** One source implies overseas-hosted
   sites still need it. Not confirmed as applying to a personal foreign-hosted
   open-source project site; no action proposed.
10. **Non-amd64 builds.** UOS targets Zhaoxin/Hygon (x86), Kunpeng/Phytium/Kylin
    (ARM), Loongson (LoongArch). Fresco ships amd64 only. If ARM64/LoongArch ever
    becomes a target: VA-API hardware-decode implementations differ per chip, so a
    software-decode fallback path becomes load-bearing, and Rust + GTK4 availability
    on those architectures needs checking. Out of scope for this plan — Deepin
    community users are overwhelmingly x86 — but record it as a known limitation in
    any store listing.

---

## 10. Checklist

- [x] ~~Verify Gitee registration is possible from outside China (§9.1)~~ — **done, registration succeeded.**
      §9.1's open question is resolved: an account can be created from outside
      mainland China. The later gates (open-source review, and the deepin store's
      identity verification) are still untested.
- [x] **Patch `install.sh` for the mirror — done.** `FRESCO_ORIGIN=github|gitee`
      selects the host, and the JSON parser is now host-agnostic (§6). Verified
      against both a pretty-printed GitHub payload and a minified Gitee one; the
      old `sed` returned the *entire JSON document* on minified input.
- [x] **Patch the update path — done.** `update::Origin` resolves the host once,
      from `FRESCO_ORIGIN` or the `~/.config/fresco/install-origin` marker that
      `install.sh` now writes. `src/update.rs`, `src/gui/updates.rs` and
      `src/daemon/notifier.rs` all route through it, and
      `scripts/fresco-update.sh` takes `--origin` as an **argument** (it runs
      under `pkexec` as root, so it can read neither the environment nor the
      invoking user's config). Covered by tests in `src/update.rs`.
**Gitee is blocked** — binding a mobile number is required for repo import and a
+91 number was rejected (§4 box). The remaining Gitee items are parked, not done:

- [ ] *(10 min, settles the open question)* On the existing email-registered Gitee
      account, try in order: create a public repo → create a Release with a `.deb`
      attached → generate a 私人令牌 (设置 → 安全设置). Whichever step first demands a
      phone is the real boundary. Gitee documents this nowhere, so the account is a
      better source than the docs.
- [ ] *(only if a mainland collaborator owns a 组织)* Confirm a phone-less member can
      push and publish releases — undocumented. Members act with their **own** token,
      so at least no credential sharing is required.
- [ ] If Gitee ever happens: correct the owner/repo slug in `update::Origin` and
      `install.sh` — both currently assume `dibbayajyoti/fresco`
- [ ] Add `GITEE_TOKEN` + `GITEE_REPO` secrets; run `draft-gitee-mirror.yml` against a tag
- [ ] End-to-end test: install with `FRESCO_ORIGIN=gitee`, then confirm the
      in-app updater queries **gitee.com** and not github.com

**Spark Store — the live path:**

- [ ] Build the `.deb` and run `lintian` on it; confirm no `bad-package-name`
- [ ] Write the Chinese listing description (crib from the metainfo.xml zh_CN text)
- [ ] Prepare icon (PNG 128–512px) and 1–5 PNG screenshots
- [ ] Decide the permanent `昵称<邮箱>` pair and app name — never change them after
- [ ] Re-read `packaging/debian` maintainer scripts (reviewers inspect them)
- [ ] Decide what the zh-CN build offers as default media sources (Pinterest is
      blocked in the mainland)
- [ ] Submit via <https://upload.deepinos.org.cn/index> (needs a Linux browser)
- [ ] Promote the draft workflow to `_gitee-mirror.yml` and wire into `publish.yml`
- [ ] Add the mainland one-liner + Gitee link to the landing page and README
- [ ] `lintian` check, then submit to Spark Store
- [ ] Test the `.deb` on deepin 23 (and 20), then submit to the deepin app store
