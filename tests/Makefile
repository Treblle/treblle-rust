.PHONY: all test clean monitoring logs help test-axum test-actix test-rocket

all: help

monitoring:
	@echo "Starting monitoring stack..."
	docker compose -f docker-compose.monitoring.yml up -d
	@echo "Monitoring stack started"
	@echo "Grafana available at http://localhost:3000 (admin/admin)"
	@echo "Prometheus available at http://localhost:9090"

# Run all tests
test:
	@echo "Running all tests..."
	docker compose -f docker-compose.test.yml up --build -d
	@echo "Tests started. Monitor progress at http://localhost:3000"

# Run specific framework tests
test-axum:
	@echo "Running Axum tests..."
	docker compose -f docker-compose.test.yml up --build -d axum-test-service mock-treblle-api k6 prometheus grafana
	@echo "Axum tests started. Monitor at http://localhost:3000"

test-actix:
	@echo "Running Actix tests..."
	docker compose -f docker-compose.test.yml up --build -d actix-test-service mock-treblle-api k6 prometheus grafana
	@echo "Actix tests started. Monitor at http://localhost:3000"

test-rocket:
	@echo "Running Rocket tests..."
	docker compose -f docker-compose.test.yml up --build -d rocket-test-service mock-treblle-api k6 prometheus grafana
	@echo "Rocket tests started. Monitor at http://localhost:3000"

logs:
	docker compose -f docker-compose.test.yml logs -f

# View logs for specific service
logs-%:
	docker compose -f docker-compose.test.yml logs -f $*

clean:
	@echo "Cleaning up..."
	docker compose -f docker-compose.test.yml down -v
	docker compose -f docker-compose.monitoring.yml down -v
	rm -rf tmp/
	@echo "Cleanup complete"

help:
	@echo "Treblle Integration Tests"
	@echo ""
	@echo "Available commands:"
	@echo "  make monitoring    - Start monitoring stack (Grafana + Prometheus)"
	@echo "  make test         - Run all framework tests"
	@echo "  make test-axum    - Run Axum framework tests"
	@echo "  make test-actix   - Run Actix framework tests"
	@echo "  make test-rocket  - Run Rocket framework tests"
	@echo "  make logs         - View all test logs"
	@echo "  make logs-<svc>   - View logs for specific service"
	@echo "                      (e.g., make logs-axum-test-service)"
	@echo "  make clean        - Clean up containers and volumes"
