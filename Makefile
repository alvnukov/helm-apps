all: deps
deps:
	werf helm dependency update charts/helm-apps
	werf helm dependency update tests/.helm
save_tests:
	cd tests; werf render --set "global._includes.apps-defaults.enabled=true" --env=prod --dev | sed '/werf.io\//d' > test_render.yaml
save_contracts_snapshot:
	werf helm dependency update tests/contracts
	werf helm template contracts tests/contracts --set global.env=production | sed '/werf.io\//d' > tests/contracts/test_render.snapshot.yaml
ci_local:
	bash scripts/ci-local.sh
fuzz_contracts:
	bash scripts/fuzz-contracts.sh --iterations 40 --seed 20260216
coverage_entities:
	bash scripts/check-entity-coverage.sh
