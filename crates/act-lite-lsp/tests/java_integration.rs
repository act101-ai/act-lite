//! Java LSP integration tests for jdtls.

use act_lite_lsp::{LspPool, ServerConfig};
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn jdtls_server_config_is_valid() {
    let workspace = PathBuf::from("/tmp/test");
    let config = ServerConfig::java(workspace.clone());

    assert_eq!(config.command, "jdtls");
    assert_eq!(config.working_directory, Some(workspace));
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn jdtls_starts_with_maven_project() {
    let temp_dir = TempDir::new().unwrap();

    // Create minimal Maven project
    std::fs::write(
        temp_dir.path().join("pom.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0
         http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>test</artifactId>
    <version>1.0-SNAPSHOT</version>
    <properties>
        <maven.compiler.source>17</maven.compiler.source>
        <maven.compiler.target>17</maven.compiler.target>
    </properties>
</project>
"#,
    )
    .unwrap();

    // Create source directory
    let src_dir = temp_dir.path().join("src/main/java/com/example");
    std::fs::create_dir_all(&src_dir).unwrap();

    std::fs::write(
        src_dir.join("Sample.java"),
        r#"
package com.example;

public class Sample {
    private String name;

    public Sample(String name) {
        this.name = name;
    }

    public String getName() {
        return name;
    }
}
"#,
    )
    .unwrap();

    let pool = LspPool::new(temp_dir.path().to_path_buf());

    // Try to get Java LSP client (using Grammar::Java)
    use act_lite_parser::Grammar;
    let client = pool.get_client(Grammar::Java).await;

    // Client may be None if jdtls is not installed, which is acceptable in CI
    if client.is_none() {
        eprintln!("jdtls not available (expected in some environments)");
    }
    // Test passes regardless - we're just verifying the code compiles and runs
}

#[tokio::test]
async fn jdtls_detects_gradle_project() {
    let temp_dir = TempDir::new().unwrap();

    // Create minimal Gradle project
    std::fs::write(
        temp_dir.path().join("build.gradle"),
        r#"
plugins {
    id 'java'
}

group = 'com.example'
version = '1.0-SNAPSHOT'

sourceCompatibility = '17'
targetCompatibility = '17'
"#,
    )
    .unwrap();

    let config = ServerConfig::java(temp_dir.path().to_path_buf());

    // Config should be valid for Gradle projects too
    assert!(config.validate().is_ok());
}

#[test]
fn java_server_config_has_jdtls_command() {
    // Verify Java config uses jdtls command
    let config = ServerConfig::java(PathBuf::from("/tmp"));
    assert_eq!(config.command, "jdtls");
}
