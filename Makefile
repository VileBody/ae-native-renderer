IMAGE ?= ae-native-renderer:dev
JOB ?= demo_static

build:
	docker build -t $(IMAGE) .

doctor:
	docker run --rm -v "$(PWD)/jobs:/work/jobs" -v "$(PWD)/fixtures:/work/fixtures" $(IMAGE) doctor

render-demo:
	docker run --rm -v "$(PWD)/jobs:/work/jobs" -v "$(PWD)/fixtures:/work/fixtures" $(IMAGE) render --scene /work/jobs/$(JOB)/scene.json --out /work/jobs/$(JOB)/out

mux-demo:
	./scripts/mux_png_to_mp4.sh jobs/$(JOB)/out/frames 30 jobs/$(JOB)/out/result.mp4

fmt:
	cargo fmt --all

check:
	cargo check --workspace
