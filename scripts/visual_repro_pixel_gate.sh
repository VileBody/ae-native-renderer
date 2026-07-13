#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 ]]; then
  echo "usage: $0 RUST_MP4 AE_MP4 OUT_DIR [FPS]" >&2
  exit 2
fi

rust_mp4=$1
ae_mp4=$2
out_dir=$3
fps=${4:-24000/1001}

mkdir -p "$out_dir"

common="[0:v]fps=${fps},scale=1080:1960,setsar=1,setpts=PTS-STARTPTS[r];[1:v]fps=${fps},scale=1080:1960,setsar=1,setpts=PTS-STARTPTS[a]"

ffmpeg -y -v error \
  -i "$rust_mp4" \
  -i "$ae_mp4" \
  -filter_complex "${common};[r][a]psnr=stats_file=${out_dir}/psnr.log" \
  -f null -

ffmpeg -y -v error \
  -i "$rust_mp4" \
  -i "$ae_mp4" \
  -filter_complex "${common};[r][a]blend=all_mode=difference,format=gray,signalstats,metadata=print:file=${out_dir}/diff_signalstats.log" \
  -f null -

ffmpeg -y -v error \
  -i "$rust_mp4" \
  -i "$ae_mp4" \
  -filter_complex "${common};[r][a]blend=all_mode=difference,eq=contrast=8:brightness=0.04,scale=490:490,tile=4x2" \
  -frames:v 1 "${out_dir}/diff_contact.jpg"

echo "pixel gate wrote: ${out_dir}"
