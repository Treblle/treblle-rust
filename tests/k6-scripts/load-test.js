import http from 'k6/http';
import { check, group, sleep } from 'k6';
import { Counter, Trend, Rate } from 'k6/metrics';
import { randomString } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';

// Custom metrics for detailed monitoring
export const metrics = {
    // Regular routes metrics
    regularRequests: new Counter('regular_requests'),
    regularResponseTime: new Trend('regular_response_time'),
    regularSuccessRate: new Rate('regular_success_rate'),

    // Monitored routes metrics (with Treblle)
    monitoredRequests: new Counter('monitored_requests'),
    monitoredResponseTime: new Trend('monitored_response_time'),
    monitoredSuccessRate: new Rate('monitored_success_rate'),

    // Content type specific metrics
    jsonRequests: new Counter('json_requests'),
    nonJsonRequests: new Counter('non_json_requests'),

    // Error tracking
    errors: new Counter('error_count'),

    // Sensitive data masking verification
    maskingVerified: new Rate('masking_verified_rate'),

    // Additional metrics for Treblle payload testing
    validTrebllePayloads: new Counter('valid_treblle_payloads'),
    invalidTrebllePayloads: new Counter('invalid_treblle_payloads'),
    payloadSizes: new Trend('payload_sizes'),
    maskedFieldsCount: new Counter('masked_fields_count'),
};

// Test configuration with scenarios
export const options = {
    scenarios: {
        regular_vs_monitored: {
            executor: 'ramping-vus',
            startVUs: 0,
            stages: [
                { duration: '30s', target: 20 },  // Ramp up
                { duration: '2m', target: 20 },   // Steady load
                { duration: '30s', target: 0 },   // Ramp down
            ],
            gracefulRampDown: '30s',
            exec: 'compareEndpoints',
        },
        stress_test: {
            executor: 'ramping-arrival-rate',
            startRate: 1,
            timeUnit: '1s',
            preAllocatedVUs: 50,
            maxVUs: 100,
            stages: [
                { duration: '1m', target: 50 },   // Ramp up load
                { duration: '30s', target: 50 },  // Steady high load
                { duration: '30s', target: 0 },   // Ramp down
            ],
            exec: 'compareEndpoints',
        },
        treblle_payload_tests: {
            executor: 'ramping-arrival-rate',
            startRate: 1,
            timeUnit: '1s',
            preAllocatedVUs: 20,
            maxVUs: 50,
            stages: [
                { duration: '30s', target: 10 },
                { duration: '1m', target: 10 },
                { duration: '30s', target: 0 },
            ],
            exec: 'testTrebllePayloads',
        },
    },
    thresholds: {
        'regular_response_time': ['p(95)<500'],
        'monitored_response_time': ['p(95)<600'],  // Allow slightly higher latency for monitored routes
        'regular_success_rate': ['rate>0.95'],
        'monitored_success_rate': ['rate>0.95'],
        'masking_verified_rate': ['rate>0.99'],    // Ensure sensitive data is consistently masked
        'error_count': ['count<10'],
        'valid_treblle_payloads': ['count>100'],
        'invalid_treblle_payloads': ['count>0'], // Ensure we test error cases
    },
};

function generateValidTrebllePayload() {
    return {
        api_key: 'test_key',
        project_id: 'test_project',
        version: 1.0,
        sdk: 'rust-treblle/0.1.0',
        data: {
            server: {
                ip: '127.0.0.1',
                timezone: 'UTC',
                software: 'nginx/1.18.0',
                signature: '',
                protocol: 'HTTP/1.1',
                os: {
                    name: 'Linux',
                    release: '5.15.0-1019-aws',
                    architecture: 'x86_64',
                },
            },
            language: {
                name: 'rust',
                version: '1.70.0',
            },
            request: {
                timestamp: new Date().toISOString(),
                ip: '192.168.1.1',
                url: 'http://api.example.com/endpoint',
                user_agent: 'Mozilla/5.0...',
                method: 'POST',
                headers: {
                    'content-type': 'application/json',
                    'user-agent': 'Mozilla/5.0...',
                },
                body: {
                    sensitive_field: 'should_be_masked',
                    credit_card: '4111-1111-1111-1111',
                    password: 'super_secret',
                    regular_field: 'visible_data',
                },
            },
            response: {
                headers: {
                    'content-type': 'application/json',
                },
                code: 200,
                size: 1234,
                load_time: 0.123,
                body: {
                    status: 'success',
                    data: { id: 123 },
                },
            },
            errors: [],
        },
    };
}

function generateInvalidTrebllePayload() {
    const invalidCases = [
        // Missing required fields
        { project_id: 'test_project', version: 1.0, sdk: 'rust-treblle/0.1.0', data: {} },
        { api_key: 'test_key', version: 1.0, sdk: 'rust-treblle/0.1.0', data: {} },

        // Invalid values
        {
            api_key: 'test_key',
            project_id: 'test_project',
            version: -1.0, // Invalid version
            sdk: 'rust-treblle/0.1.0',
            data: {},
        },

        // Malformed data structure
        {
            api_key: 'test_key',
            project_id: 'test_project',
            version: 1.0,
            sdk: 'rust-treblle/0.1.0',
            data: 'not_an_object',
        },

        // Empty required strings
        {
            api_key: '',
            project_id: '',
            version: 1.0,
            sdk: '',
            data: {},
        },
    ];

    return randomItem(invalidCases);
}

function generateTestData(includesSensitive = true) {
    const data = {
        message: `Test message ${randomString(8)}`,
        timestamp: new Date().toISOString(),
        nested: {
            field: randomString(5),
            array: [1, 2, 3],
        },
    };

    if (includesSensitive) {
        data.password = 'super_secret_123';
        data.credit_card = '4111-1111-1111-1111';
        data.api_key = 'sk_test_123456789';
    }

    return data;
}

export function testTrebllePayloads() {
    group('Treblle Payload Tests', () => {
        // Test valid payload
        {
            const payload = generateValidTrebllePayload();
            const response = http.post(
                'http://mock-treblle-api:4321/',
                JSON.stringify(payload),
                {
                    headers: { 'Content-Type': 'application/json' },
                }
            );

            metrics.validTrebllePayloads.add(1);
            metrics.payloadSizes.add(JSON.stringify(payload).length);

            // Count masked fields in the response
            const maskedCount = (JSON.stringify(response.json())
                .match(/\*\*\*\*\*/g) || []).length;
            metrics.maskedFieldsCount.add(maskedCount);

            check(response, {
                'valid payload status is 200': (r) => r.status === 200,
                'valid payload response is success': (r) => r.json('status') === 'success',
                'sensitive data is masked': (r) => maskedCount > 0,
            });
        }

        // Test invalid payload
        {
            const payload = generateInvalidTrebllePayload();
            const response = http.post(
                'http://mock-treblle-api:4321/',
                JSON.stringify(payload),
                {
                    headers: { 'Content-Type': 'application/json' },
                }
            );

            metrics.invalidTrebllePayloads.add(1);

            check(response, {
                'invalid payload status is 200': (r) => r.status === 200,
                'invalid payload response is error': (r) => r.json('status') === 'error',
                'error message exists': (r) => r.json('errors') !== null,
            });
        }

        sleep(1);
    });
}

// Main test function comparing regular vs monitored endpoints
export function compareEndpoints() {
    const baseUrl = 'http://axum-test-service:8082';

    group('JSON Endpoints Comparison', () => {
        const payload = generateTestData(true);

        // Test regular endpoint
        {
            const regularResponse = http.post(
                `${baseUrl}/api/json`,
                JSON.stringify(payload),
                { headers: { 'Content-Type': 'application/json' } }
            );

            metrics.regularRequests.add(1);
            metrics.regularResponseTime.add(regularResponse.timings.duration);
            metrics.regularSuccessRate.add(regularResponse.status === 200);

            check(regularResponse, {
                'regular status is 200': (r) => r.status === 200,
                'regular response is valid': (r) => r.json('message') !== undefined,
            });
        }

        // Test monitored endpoint
        {
            const monitoredResponse = http.post(
                `${baseUrl}/api/with-treblle/json`,
                JSON.stringify(payload),
                { headers: { 'Content-Type': 'application/json' } }
            );

            metrics.monitoredRequests.add(1);
            metrics.monitoredResponseTime.add(monitoredResponse.timings.duration);
            metrics.monitoredSuccessRate.add(monitoredResponse.status === 200);

            const success = check(monitoredResponse, {
                'monitored status is 200': (r) => r.status === 200,
                'monitored response is valid': (r) => r.json('message') !== undefined,
                'sensitive data is masked': (r) => {
                    const body = r.json();
                    return !body.toString().includes('super_secret') &&
                           !body.toString().includes('4111-1111-1111-1111');
                },
            });

            metrics.maskingVerified.add(success);
        }

        sleep(1);
    });
}
