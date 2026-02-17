//! Integration test binary for verifying junctions survive Windows Container layer snapshots.
//!
//! Usage:
//!   container_layer create <target_dir> <junction_path>
//!   container_layer verify <junction_path> <expected_target>
//!
//! In a Dockerfile, run `create` in one RUN step and `verify` in the next.
//! If PrintName is empty, the junction will break during layer serialization.

use std::env;
use std::fs;
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        // When run by `cargo test` with no arguments, just exit successfully.
        // This binary is meant to be invoked inside a Docker container with
        // explicit create/verify subcommands.
        eprintln!("container_layer: no command given, nothing to do (this is OK for cargo test)");
        return;
    }

    match args[1].as_str() {
        "create" => {
            if args.len() != 4 {
                eprintln!("Usage: {} create <target_dir> <junction_path>", args[0]);
                process::exit(1);
            }
            let target = Path::new(&args[2]);
            let junction = Path::new(&args[3]);

            // Create the target directory
            fs::create_dir_all(target).expect("failed to create target directory");

            // Create a marker file so we can verify the junction resolves correctly
            let marker = target.join("marker.txt");
            fs::write(&marker, "junction-test-ok").expect("failed to write marker file");

            // Create the junction
            junction::create(target, junction).expect("failed to create junction");

            // Verify it works in this layer
            assert!(
                junction::exists(junction).expect("failed to check junction"),
                "junction should exist after creation"
            );
            let resolved = junction::get_target(junction).expect("failed to get junction target");
            eprintln!("Created junction: {:?} -> {:?}", junction, resolved);

            let marker_via_junction = junction.join("marker.txt");
            let content = fs::read_to_string(&marker_via_junction).expect("failed to read marker via junction");
            assert_eq!(content, "junction-test-ok", "marker content mismatch in creation layer");

            eprintln!("Junction created and verified in this layer.");
        }
        "verify" => {
            if args.len() != 4 {
                eprintln!("Usage: {} verify <junction_path> <expected_target>", args[0]);
                process::exit(1);
            }
            let junction = Path::new(&args[2]);
            let expected_target = Path::new(&args[3]);

            // Check the junction still exists after layer snapshot
            assert!(
                junction.exists(),
                "junction path does not exist in this layer: {:?}",
                junction
            );

            assert!(
                junction::exists(junction).expect("failed to check junction"),
                "junction is no longer recognized as a junction point after layer snapshot"
            );

            let resolved = junction::get_target(junction).expect("failed to get junction target");
            eprintln!("Junction in new layer: {:?} -> {:?}", junction, resolved);

            // The resolved target should match the expected target
            let resolved_canonical = fs::canonicalize(&resolved).unwrap_or_else(|_| resolved.clone());
            let expected_canonical =
                fs::canonicalize(expected_target).unwrap_or_else(|_| expected_target.to_path_buf());

            // Strip \\?\ verbatim prefix for comparison
            let resolved_str = resolved_canonical.to_string_lossy();
            let resolved_str = resolved_str.strip_prefix(r"\\?\").unwrap_or(&resolved_str);
            let expected_str = expected_canonical.to_string_lossy();
            let expected_str = expected_str.strip_prefix(r"\\?\").unwrap_or(&expected_str);

            assert_eq!(
                resolved_str, expected_str,
                "junction target does not match expected target after layer snapshot"
            );

            // Verify the marker file is accessible through the junction
            let marker_via_junction = junction.join("marker.txt");
            let content = fs::read_to_string(&marker_via_junction)
                .expect("failed to read marker file via junction in new layer - junction is broken!");
            assert_eq!(
                content, "junction-test-ok",
                "marker content mismatch after layer snapshot"
            );

            eprintln!("Junction survived layer snapshot successfully!");
        }
        other => {
            eprintln!("Unknown command: {}", other);
            process::exit(1);
        }
    }
}
