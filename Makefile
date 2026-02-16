all: deps
deps:
	werf helm dependency update charts/helm-apps
	werf helm dependency update tests/.helm
save_tests:
	cd tests; werf render --set "global._includes.apps-defaults.enabled=true" --env=prod --dev | sed '/werf.io\//d' > test_render.yaml
ci_local:
	bash scripts/ci-local.sh
fuzz_contracts:
	bash scripts/fuzz-contracts.sh --iterations 40 --seed 20260216
