import http from 'k6/http';
import { check, group, sleep } from 'k6';
import { Counter, Trend, Rate, Gauge } from 'k6/metrics';
import { randomString } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';

/**
 * Custom metrics aligned with Prometheus and Grafana dashboard
 */
export const metrics = {
    // Core request metrics
    requestsTotal: new Counter('requests_total'),
    requestDurationMs: new Trend('request_duration_ms'),
    requestSuccessRate: new Rate('request_success_rate'),
    requestsInProgress: new Gauge('requests_in_progress'),

    // Request phases timing
    requestSendingMs: new Trend('request_sending_ms'),
    requestWaitingMs: new Trend('request_waiting_ms'),
    requestReceivingMs: new Trend('request_receiving_ms'),

    // Middleware performance
    middlewareOverheadMs: new Trend('middleware_overhead_ms'),
    mockApiLatencyMs: new Trend('mock_api_latency_ms'),

    // Data processing
    sensitiveFieldsTotal: new Counter('sensitive_fields_total'),
    maskedFieldsTotal: new Counter('masked_fields_total'),
    maskingSuccessRate: new Rate('masking_success_rate'),

    // Errors
    errorsTotal: new Counter('errors_total'),
    validationErrorsTotal: new Counter('validation_errors_total'),

    // Payload metrics
    payloadSizeBytes: new Trend('payload_size_bytes'),

    // Runtime correlation metrics
    tokioTaskUtilization: new Gauge('tokio_task_utilization'),
    tokioPollLatencyMs: new Trend('tokio_poll_latency_ms'),

    // System correlation metrics
    cpuUsagePercent: new Gauge('cpu_usage_percent'),
    memoryUsagePercent: new Gauge('memory_usage_percent')
};

/**
 * Test configuration
 */
const config = {
    middleware: __ENV.MIDDLEWARE_TYPE || 'axum',
    baseUrls: {
        axum: 'http://axum-test-service:8082',
        actix: 'http://actix-test-service:8083',
        rocket: 'http://rocket-test-service:8084',
    },
    mockApi: 'http://mock-treblle-api:4321',
    defaultHeaders: {
        'Content-Type': 'application/json',
    },
};

/**
 * Load test scenarios configuration
 */
export const options = {
    scenarios: {
        // Baseline performance testing
        baseline_performance: {
            executor: 'ramping-arrival-rate',
            startRate: 1,
            timeUnit: '1s',
            preAllocatedVUs: 20,
            maxVUs: 50,
            stages: [
                { duration: '30s', target: 5 },   // Warm up
                { duration: '1m', target: 10 },   // Baseline load
                { duration: '30s', target: 0 },   // Cool down
            ],
        },

        // Data masking tests
        data_masking: {
            executor: 'constant-arrival-rate',
            rate: 10,
            timeUnit: '1s',
            duration: '1m',
            preAllocatedVUs: 20,
            maxVUs: 40,
            startTime: '2m30s',
            tags: { test_type: 'masking' }
        },

        // Performance impact testing
        stress_test: {
            executor: 'ramping-arrival-rate',
            startRate: 10,
            timeUnit: '1s',
            preAllocatedVUs: 50,
            maxVUs: 100,
            stages: [
                { duration: '30s', target: 20 },  // Ramp up
                { duration: '1m', target: 50 },   // Stress test
                { duration: '30s', target: 100 }, // Peak load
                { duration: '30s', target: 0 },   // Cool down
            ],
            startTime: '4m',
            tags: { test_type: 'stress' }
        },

        spike_test: {
            executor: 'ramping-arrival-rate',
            startRate: 50,
            timeUnit: '1s',
            preAllocatedVUs: 100,
            maxVUs: 200,
            stages: [
                { duration: '10s', target: 200 },
                { duration: '30s', target: 200 },
                { duration: '10s', target: 50 }
            ],
            startTime: '6m',
            tags: { test_type: 'spike' }
        },

        long_term_stability: {
            executor: 'constant-arrival-rate',
            rate: 30,
            timeUnit: '1s',
            duration: '10m',
            preAllocatedVUs: 30,
            maxVUs: 60,
            startTime: '8m',
            tags: { test_type: 'stability' }
        }
    },

    thresholds: {
        'request_duration_ms{type:regular}': ['p(95)<500'],
        'request_duration_ms{type:monitored}': ['p(95)<600'],
        'middleware_overhead_ms': ['p(95)<100'],
        'request_success_rate{type:regular}': ['rate>0.95'],
        'request_success_rate{type:monitored}': ['rate>0.95'],
        'masking_success_rate': ['rate>0.99'],
        'errors_total': ['count<10'],
        'mock_api_latency_ms': ['p(95)<200'],
        'validation_errors_total': ['count<5'],
        'requests_in_progress{type:monitored}': ['value<50']
    }
};

/**
 * Generates sensitive test data
 */
function generateSensitiveData() {
    return {
        password: `secret_${randomString(8)}`,
        credit_card: '4111-1111-1111-1111',
        ssn: '123-45-6789',
        api_key: randomString(32),
        nested: {
            secret_key: randomString(16),
            card_number: '5555-5555-5555-5555'
        }
    };
}

/**
 * Generates test request payload
 */
function generateRequest(includeSensitive = false) {
    const payload = {
        message: `Test message ${randomString(8)}`,
        delay_ms: Math.floor(Math.random() * 50),  // 0-50ms random delay
        sensitive_data: includeSensitive ? generateSensitiveData() : null,
    };

    metrics.payloadSizeBytes.add(JSON.stringify(payload).length, { type: 'request' });
    return payload;
}

/**
 * Setup function runs once before the test
 */
export function setup() {
    const baseUrl = config.baseUrls[config.middleware];
    if (!baseUrl) {
        throw new Error(`Invalid middleware type: ${config.middleware}`);
    }

    // Maximum number of retries and delay between retries
    const maxRetries = 10;
    const retryDelay = 2;
    let retries = 0;

    while (retries < maxRetries) {
        try {
            console.log(`Attempt ${retries + 1} of ${maxRetries} to connect to services...`);

            // Health checks
            const serviceHealth = http.get(`${baseUrl}/health`);
            const mockApiHealth = http.get(`${config.mockApi}/health`);
            const prometheusHealth = http.get('http://prometheus:9090/-/ready');

            if (serviceHealth.status === 200 && mockApiHealth.status === 200 && prometheusHealth.status === 200) {
                console.log('All services are healthy and ready');
                return { baseUrl };
            }

            throw new Error('Service health checks failed');
        } catch (error) {
            console.error(`Connection attempt failed: ${error.message}`);
            retries++;

            if (retries < maxRetries) {
                console.log(`Retrying in ${retryDelay} seconds...`);
                sleep(retryDelay);
            }
        }
    }

    throw new Error(`Failed to connect to services after ${maxRetries} attempts`);
}

export default function(data) {
    const baseUrl = data.baseUrl;

    group('Regular API Endpoints', () => {
        metrics.requestsInProgress.add(1, { type: 'regular' });
        const payload = generateRequest(false);
        const start = new Date();

        const response = http.post(
            `${baseUrl}/api/json`,
            JSON.stringify(payload),
            {
                headers: config.defaultHeaders,
                tags: { endpoint_type: 'regular' }
            }
        );

        // Track request phases timing
        metrics.requestSendingMs.add(response.timings.sending);
        metrics.requestWaitingMs.add(response.timings.waiting);
        metrics.requestReceivingMs.add(response.timings.receiving);


        // Track total duration
        const duration = (new Date() - start);
        metrics.requestDurationMs.add(duration, { type: 'regular' });
        metrics.requestsTotal.add(1, { type: 'regular' });

        // Track response size
        if (response.body) {
            metrics.payloadSizeBytes.add(response.body.length, { type: 'response' });
        }

        const success = check(response, {
            'regular endpoint returns 200': (r) => r.status === 200,
            'regular response is valid JSON': (r) => {
                try {
                    return JSON.parse(r.body) !== null;
                } catch (e) {
                    return false;
                }
            },
        });

        if (success) {
            metrics.requestSuccessRate.add(1, { type: 'regular' });
        } else {
            metrics.errorsTotal.add(1, { type: 'request' });
        }

        metrics.requestsInProgress.add(-1, { type: 'regular' });
    });

    group('Monitored API Endpoints', () => {
        metrics.requestsInProgress.add(1, { type: 'monitored' });
        const payload = generateRequest(true);
        const start = new Date();

        const response = http.post(
            `${baseUrl}/api/with-treblle/json`,
            JSON.stringify(payload),
            {
                headers: config.defaultHeaders,
                tags: { endpoint_type: 'monitored' }
            }
        );

        // Track request phases
        metrics.requestSendingMs.add(response.timings.sending);
        metrics.requestWaitingMs.add(response.timings.waiting);
        metrics.requestReceivingMs.add(response.timings.receiving);

        const duration = (new Date() - start);
        metrics.requestDurationMs.add(duration, { type: 'monitored' });
        metrics.requestsTotal.add(1, { type: 'monitored' });

        // Track response size
        if (response.body) {
            metrics.payloadSizeBytes.add(response.body.length, { type: 'response' });
        }

        const success = check(response, {
            'monitored endpoint returns 200': (r) => r.status === 200,
            'monitored response is valid': (r) => {
                try {
                    const body = JSON.parse(r.body);

                    // Verify sensitive data masking
                    let maskingSuccess = true;
                    if (payload.sensitive_data) {
                        const sensitiveFields = countSensitiveFields(payload.sensitive_data);
                        metrics.sensitiveFieldsTotal.add(sensitiveFields);

                        const maskedFields = countMaskedFields(body.sensitive_data);
                        metrics.maskedFieldsTotal.add(maskedFields);

                        maskingSuccess = sensitiveFields === maskedFields;
                        if (maskingSuccess) {
                            metrics.maskingSuccessRate.add(1);
                        } else {
                            metrics.errorsTotal.add(1, { type: 'masking' });
                        }
                    }

                    return maskingSuccess;
                } catch (e) {
                    console.error('Response parsing failed:', e);
                    metrics.errorsTotal.add(1, { type: 'validation' });
                    return false;
                }
            }
        });

        if (success) {
            metrics.requestSuccessRate.add(1, { type: 'monitored' });

            // Calculate and record middleware overhead
            const regularBaseline = metrics.requestDurationMs.values['type:regular']?.avg || 0;
            const overhead = duration - regularBaseline;
            if (overhead > 0) {
                metrics.middlewareOverheadMs.add(overhead);
            }
        } else {
            metrics.errorsTotal.add(1, { type: 'request' });
        }

        metrics.requestsInProgress.add(-1, { type: 'monitored' });
    });

    // Helper functions for sensitive data analysis
    function countSensitiveFields(obj, count = 0) {
        if (!obj) return count;
        if (typeof obj !== 'object') return count;

        for (const key of Object.keys(obj)) {
            if (isSensitiveField(key)) count++;
            if (typeof obj[key] === 'object') {
                count = countSensitiveFields(obj[key], count);
            }
        }
        return count;
    }

    function countMaskedFields(obj, count = 0) {
        if (!obj) return count;
        if (typeof obj !== 'object') return count;

        for (const key of Object.keys(obj)) {
            if (isSensitiveField(key) && obj[key] === '*****') count++;
            if (typeof obj[key] === 'object') {
                count = countMaskedFields(obj[key], count);
            }
        }
        return count;
    }

    function isSensitiveField(field) {
        const patterns = [
            'password', 'pwd', 'secret', 'pass',
            'credit', 'card', 'ccv', 'cvv', 'cvc',
            'ssn', 'social', 'key'
        ];
        return patterns.some(pattern => field.toLowerCase().includes(pattern));
    }

    // Add a small delay between iterations to prevent overwhelming the service
    sleep(1);
}

/**
 * Teardown function for final metric validation
 */
export function teardown(data) {
    const mockApiMetrics = http.get(`${config.mockApi}/metrics`);

    check(mockApiMetrics, {
        'mock API metrics available': (r) => r.status === 200,
        'all requests properly processed': (r) => {
            try {
                const metrics = JSON.parse(r.body);
                const validationSuccess = metrics.validation.total_requests === metrics.validation.valid_requests;
                const maskingSuccess = metrics.validation.masked_fields_count >= metrics.validation.total_requests;

                if (!validationSuccess) {
                    console.error('Validation mismatch:', metrics.validation);
                }
                if (!maskingSuccess) {
                    console.error('Masking mismatch:', metrics.validation);
                }

                return validationSuccess && maskingSuccess;
            } catch (e) {
                console.error('Error parsing mock API metrics:', e);
                return false;
            }
        }
    });

    // Output test summary
    console.log('\nTest Summary:');
    console.log('=============');
    console.log('Regular Requests:', metrics.requestsTotal.values['type:regular']?.count || 0);
    console.log('Monitored Requests:', metrics.requestsTotal.values['type:monitored']?.count || 0);
    console.log('Average Middleware Overhead:',
        (metrics.middlewareOverheadMs.avg * 1000).toFixed(2), 'ms');
    console.log('Masking Success Rate:',
        (metrics.maskingSuccessRate.rate * 100).toFixed(2), '%');
    console.log('Total Errors:',
        Object.values(metrics.errorsTotal.values).reduce((a, b) => a + b.count, 0));
}
