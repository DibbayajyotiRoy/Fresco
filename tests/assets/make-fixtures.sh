#!/usr/bin/env bash
# Generate test media fixtures for the audio + fidelity harnesses (T0.2).
#
# All output goes to tests/assets/generated/ (gitignored — fixtures are
# regenerated, never committed). Runs offline in under 2 minutes.
#
# Fixtures:
#   video-1080p-h264.mp4    5s testsrc2 loop, H.264 yuv420p
#   video-1080p-vp9.webm    5s testsrc2 loop, VP9
#   video-4k-h264.mp4       5s testsrc2 loop, H.264, 3840x2160
#   video-8k-h264.mp4       3s testsrc2 loop, H.264, 7680x4320 (ultrafast)
#   audio-sine-1080p.mp4    5s testsrc2 + 440Hz sine audio track
#   checkerboard-4k.mp4     5s static 1px checkerboard, near-lossless (qp 0)
#   zoneplate-8k.mp4        3s static zone plate (geq), for moire/aliasing checks
#   gradient-8bit.mp4       5s smooth horizontal gradient, yuv420p (banding check)
#   gradient-10bit.mp4      5s same gradient, yuv420p10le via libx265
#   broken.mp4              truncated H.264 file (decode-error handling)

set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
out="$here/generated"
mkdir -p "$out"

FF=(ffmpeg -hide_banner -loglevel error -y)

echo "==> 1080p H.264"
"${FF[@]}" -f lavfi -i "testsrc2=size=1920x1080:rate=30:duration=5" \
  -c:v libx264 -preset veryfast -pix_fmt yuv420p "$out/video-1080p-h264.mp4"

echo "==> 1080p VP9"
"${FF[@]}" -f lavfi -i "testsrc2=size=1920x1080:rate=30:duration=5" \
  -c:v libvpx-vp9 -deadline realtime -cpu-used 8 -b:v 2M "$out/video-1080p-vp9.webm"

echo "==> 4K H.264"
"${FF[@]}" -f lavfi -i "testsrc2=size=3840x2160:rate=30:duration=5" \
  -c:v libx264 -preset ultrafast -pix_fmt yuv420p "$out/video-4k-h264.mp4"

echo "==> 8K H.264"
"${FF[@]}" -f lavfi -i "testsrc2=size=7680x4320:rate=24:duration=3" \
  -c:v libx264 -preset ultrafast -pix_fmt yuv420p "$out/video-8k-h264.mp4"

echo "==> 1080p + 440Hz sine audio"
"${FF[@]}" -f lavfi -i "testsrc2=size=1920x1080:rate=30:duration=5" \
  -f lavfi -i "sine=frequency=440:duration=5" \
  -c:v libx264 -preset veryfast -pix_fmt yuv420p -c:a aac -shortest \
  "$out/audio-sine-1080p.mp4"

echo "==> 4K 1px checkerboard (near-lossless)"
# Luma-only 1px checkerboard survives 4:2:0 chroma subsampling; qp 0 keeps it exact.
"${FF[@]}" -f lavfi -i "nullsrc=size=3840x2160:rate=10:duration=5" \
  -vf "geq=lum='255*mod(X+Y,2)':cb=128:cr=128" \
  -c:v libx264 -preset ultrafast -qp 0 -pix_fmt yuv420p "$out/checkerboard-4k.mp4"

echo "==> 8K zone plate (aliasing/moire probe)"
"${FF[@]}" -f lavfi -i "nullsrc=size=7680x4320:rate=10:duration=3" \
  -vf "geq=lum='128+127*sin((pow(X-W/2,2)+pow(Y-H/2,2))/(W/4))':cb=128:cr=128" \
  -c:v libx264 -preset ultrafast -qp 0 -pix_fmt yuv420p "$out/zoneplate-8k.mp4"

echo "==> gradient 8-bit"
"${FF[@]}" -f lavfi -i "gradients=size=3840x2160:rate=10:duration=5:x0=0:y0=1080:x1=3840:y1=1080:c0=black:c1=white:nb_colors=2" \
  -c:v libx264 -preset ultrafast -qp 0 -pix_fmt yuv420p "$out/gradient-8bit.mp4"

echo "==> gradient 10-bit"
"${FF[@]}" -f lavfi -i "gradients=size=3840x2160:rate=10:duration=5:x0=0:y0=1080:x1=3840:y1=1080:c0=black:c1=white:nb_colors=2" \
  -c:v libx265 -preset ultrafast -x265-params "qp=0:log-level=error" -pix_fmt yuv420p10le \
  "$out/gradient-10bit.mp4"

echo "==> broken file (truncated)"
head -c 40000 "$out/video-1080p-h264.mp4" > "$out/broken.mp4"

echo "==> verify with ffprobe"
probe() { # file expected_WxH expect_audio(yes/no)
  local f="$1" want="$2" audio="$3"
  local got
  got="$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=s=x:p=0 "$f")"
  if [ "$got" != "$want" ]; then echo "FAIL: $f is $got, want $want"; exit 1; fi
  local astreams
  astreams="$(ffprobe -v error -select_streams a -show_entries stream=index -of csv=p=0 "$f" | wc -l)"
  if [ "$audio" = yes ] && [ "$astreams" -lt 1 ]; then echo "FAIL: $f has no audio"; exit 1; fi
  if [ "$audio" = no ] && [ "$astreams" -gt 0 ]; then echo "FAIL: $f unexpectedly has audio"; exit 1; fi
  echo "ok: $f ($got, audio=$audio)"
}
probe "$out/video-1080p-h264.mp4" 1920x1080 no
probe "$out/video-1080p-vp9.webm" 1920x1080 no
probe "$out/video-4k-h264.mp4"    3840x2160 no
probe "$out/video-8k-h264.mp4"    7680x4320 no
probe "$out/audio-sine-1080p.mp4" 1920x1080 yes
probe "$out/checkerboard-4k.mp4"  3840x2160 no
probe "$out/zoneplate-8k.mp4"     7680x4320 no
probe "$out/gradient-8bit.mp4"    3840x2160 no
probe "$out/gradient-10bit.mp4"   3840x2160 no
pixfmt10="$(ffprobe -v error -select_streams v:0 -show_entries stream=pix_fmt -of csv=p=0 "$out/gradient-10bit.mp4")"
[ "$pixfmt10" = "yuv420p10le" ] || { echo "FAIL: 10-bit fixture is $pixfmt10"; exit 1; }
echo "ok: gradient-10bit.mp4 pix_fmt=$pixfmt10"
# broken.mp4 must exist and fail a full decode
if ffmpeg -v error -i "$out/broken.mp4" -f null - 2>/dev/null; then
  echo "FAIL: broken.mp4 decoded cleanly"; exit 1
fi
echo "ok: broken.mp4 fails decode as intended"

echo "All fixtures generated in $out"
