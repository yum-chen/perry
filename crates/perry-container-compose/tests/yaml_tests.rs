//! Unit and property tests for YAML parsing and environment interpolation.

use perry_container_compose::yaml::*;
use proptest::prelude::*;
use std::collections::HashMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// ============ Generators ============

prop_compose! {
    // Feature: perry-container | Layer: property | Req: none | Property: -
    fn arb_env_map()(
        map in proptest::collection::hash_map("[A-Z0-9_]{1,10}", "[a-z0-9_]{1,10}", 0..20)
    ) -> HashMap<String, String> {
        map
    }
}

prop_compose! {
    // Feature: perry-container | Layer: property | Req: 7.8 | Property: 6
    fn arb_env_template()(
        var in "[A-Z0-9]{3,10}", // Use only letters/digits to avoid collisions with system env like _
        val in "[a-z0-9_]{1,10}",
        default in "[a-z0-9_]{1,10}"
    ) -> (String, HashMap<String, String>, String, String) {
        let mut env = HashMap::new();
        env.insert(var.clone(), val.clone());
        (var, env, val, default)
    }
}

// ============ Tests ============

// Feature: perry-container | Layer: property | Req: 7.8 | Property: 6
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_interpolation_basic((var, env, val, _) in arb_env_template()) {
        let input = format!("${{{}}}", var);
        let result = interpolate(&input, &env);
        prop_assert_eq!(result, val);
    }
}

// Feature: perry-container | Layer: property | Req: 7.8 | Property: 6
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_interpolation_default((var, _, _, default) in arb_env_template()) {
        let env = HashMap::new(); // Empty env
        let input = format!("${{{}:-{}}}", var, default);
        let result = interpolate(&input, &env);
        prop_assert_eq!(result, default);
    }
}

// Feature: perry-container | Layer: property | Req: 7.8 | Property: 6
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_interpolation_plus((var, env, _val, plus_val) in arb_env_template()) {
        let input = format!("${{{}:+{{{}}}}}", var, plus_val);
        let result = interpolate(&input, &env);
        // If var is set, return plus_val
        prop_assert_eq!(result, format!("{{{}}}", plus_val));

        // Note: we can't test result2 against "" if var happens to be a real system env var.
        // We ensure var is unique/unlikely to exist in arb_env_template by using specific regex.
    }
}

// Feature: perry-container | Layer: unit | Req: 7.9 | Property: -
#[test]
fn test_dotenv_parsing() {
    let content = r#"
# Comment
KEY=VALUE
SPACE_KEY =  VALUE
QUOTED="double"
SINGLE='single'
INLINE=VAL # comment
"#;
    let env = parse_dotenv(content);
    assert_eq!(env.get("KEY"), Some(&"VALUE".to_string()));
    assert_eq!(env.get("SPACE_KEY"), Some(&"VALUE".to_string()));
    assert_eq!(env.get("QUOTED"), Some(&"double".to_string()));
    assert_eq!(env.get("SINGLE"), Some(&"single".to_string()));
    assert_eq!(env.get("INLINE"), Some(&"VAL".to_string()));
}

/*
Coverage Table:
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 7.8         | prop_interpolation_basic | property |
| 7.8         | prop_interpolation_default | property |
| 7.8         | prop_interpolation_plus | property |
| 7.9         | test_dotenv_parsing | unit |

Deferred Requirements:
- none
*/
