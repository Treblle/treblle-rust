//! Constant values used across all Treblle integrations.

pub const MAX_BODY_SIZE: usize = 10 * 1024 * 1024; // 10MB

// HTTP-related constants
pub mod http {
    use std::time::Duration;

    pub const HEADER_CONTENT_TYPE: &str = "Content-Type";
    pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
}

// Default patterns moved to a separate module for clarity
pub mod defaults {
    pub const API_URLS: [&str; 3] = [
        "https://rocknrolla.treblle.com",
        "https://punisher.treblle.com",
        "https://sicario.treblle.com",
    ];

    /// Default fields to mask (exact matches)
    pub const DEFAULT_MASKED_FIELDS: [&str; 15] = [
        // Basic security fields
        "password",
        "pwd",
        "secret",
        "api_key",
        "token",
        // Payment-related
        "card_number",
        "card_cvc",
        "card_cvv",
        "cvc",
        "cvv",
        "credit_card",
        "debit_card",
        "card_pin",
        // Personal data
        "ssn",
        "social_security",
    ];

    /// Default regex patterns for field masking
    pub const DEFAULT_MASKED_FIELDS_REGEX: &str = r"(?xi)
        # Authentication & Security
        (
            password[_-]?\w*           | # password, password_hash, etc.
            auth[_-]token              | # auth_token, auth-token
            api[_-]?key[_-]?\w*        | # api_key, apikey_test
            access[_-]token[_-]?\w*    | # access_token_secret
            secret[_-]?\w*             | # secret, secret_key
            private[_-]key             | # private_key
            salt[_-]?\w*                 # salt, salt_value
        ) |

        # Payment Information
        (
            card[_-]?number           | # card_number, cardnumber
            cc[_-]?\w*                | # cc, cc_num
            cvv\d*                    | # cvv, cvv2
            cvc\d*                    | # cvc, cvc2
            pin[_-]?code              | # pin_code
            account[_-]?number          # account_number
        ) |

        # Personal/Sensitive Information
        (
            ssn                       | # Social Security Number
            social[_-]security[_-]?\w*| # social_security_number
            tax[_-]id                 | # tax_id
            passport[_-]?\w*          | # passport, passport_no
            driver[_-]?license        | # driver_license
            birth[_-]?date            | # birth_date
            dob                         # date of birth
        ) |

        # Contact Information
        (
            phone[_-]?\w*             | # phone, phone_number
            mobile[_-]?\w*            | # mobile, mobile_number
            email[_-]?address?          # email, email_address
        )";

    /// Default routes to ignore (exact matches)
    pub const DEFAULT_IGNORED_ROUTES: [&str; 12] = [
        // Health checks
        "/health", "/healthz", "/ping", "/ready", "/live", // Monitoring
        "/metrics", "/stats", "/monitor", "/status", // Debug/Dev
        "/_debug", "/debug", "/dev",
    ];

    /// Default regex patterns for route ignoring
    pub const DEFAULT_IGNORED_ROUTES_REGEX: &str = r"(?xi)
        # Case-insensitive and free-spacing mode for readability
        ^/?(
            # Common monitoring and health endpoints
            (health|alive|ready)/(check|status|ping) |

            # Debug and development routes
            debug/.*                                |
            _debug/.*                               |
            dev/.*                                  |

            # Admin and internal routes
            admin/.*                                |
            internal/.*                             |
            _internal/.*                            |

            # Monitoring and metrics
            prometheus/.*                           |
            metrics/.*                              |
            monitoring/.*                           |

            # API documentation
            swagger/.*                              |
            openapi/.*                              |
            docs/.*                                 |

            # Test routes
            test/.*                                 |
            mock/.*
        )/?$  # Optional trailing slash";
}
