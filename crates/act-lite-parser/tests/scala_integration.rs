//! Integration tests for Scala language support.

use act_lite_parser::{Grammar, SourceFile, execute_references, execute_skeleton, execute_symbols};

/// Helper to create a SourceFile from Scala content.
fn create_scala_source(content: &str) -> SourceFile {
    SourceFile::from_content("test.scala", content.to_string(), Grammar::Scala).unwrap()
}

#[test]
fn parse_case_class() {
    let content = r#"
case class User(
  id: Long,
  name: String,
  email: Option[String] = None
)
"#;
    let _source = create_scala_source(content);
    // Parsing succeeded if SourceFile was created
}

#[test]
fn parse_trait() {
    let content = r#"
trait Repository[F[_], E, ID] {
  def findById(id: ID): F[Option[E]]
  def save(entity: E): F[E]
  def delete(id: ID): F[Unit]
}
"#;
    let _source = create_scala_source(content);
    // Parsing succeeded if SourceFile was created
}

#[test]
fn parse_object_companion() {
    let content = r#"
case class Task(id: Long, title: String)

object Task {
  def empty: Task = Task(0, "")
  implicit val ordering: Ordering[Task] = Ordering.by(_.id)
}
"#;
    let _source = create_scala_source(content);
    // Parsing succeeded if SourceFile was created
}

#[test]
fn parse_pattern_matching() {
    let content = r#"
sealed trait Status
case object Pending extends Status
case object InProgress extends Status
case class Completed(at: String) extends Status

def process(status: Status): String = status match {
  case Pending => "pending"
  case InProgress => "in_progress"
  case Completed(at) => s"completed at $at"
}
"#;
    let _source = create_scala_source(content);
    // Parsing succeeded if SourceFile was created
}

#[test]
fn parse_for_comprehension() {
    let content = r#"
def processAll(service: Service): Future[List[String]] =
  for {
    tasks <- service.findAll
    completed = tasks.filter(_.isComplete)
    titles = completed.map(_.title)
  } yield titles
"#;
    let _source = create_scala_source(content);
    // Parsing succeeded if SourceFile was created
}

#[test]
fn parse_higher_kinded_types() {
    let content = r#"
trait Functor[F[_]] {
  def map[A, B](fa: F[A])(f: A => B): F[B]
}

trait Monad[F[_]] extends Functor[F] {
  def pure[A](a: A): F[A]
  def flatMap[A, B](fa: F[A])(f: A => F[B]): F[B]
}
"#;
    let _source = create_scala_source(content);
    // Parsing succeeded if SourceFile was created
}

#[test]
fn parse_implicit_class() {
    let content = r#"
implicit class StringOps(val s: String) extends AnyVal {
  def toSlug: String = s.toLowerCase.replaceAll("\\s+", "-")
}
"#;
    let _source = create_scala_source(content);
    // Parsing succeeded if SourceFile was created
}

#[test]
fn parse_type_alias() {
    let content = r#"
type UserId = Long
type Callback[T] = T => Unit
type Result[E, A] = Either[E, A]
"#;
    let _source = create_scala_source(content);
    // Parsing succeeded if SourceFile was created
}

#[test]
fn skeleton_query_finds_declarations() {
    let content = r#"
package com.example

import scala.concurrent.Future

case class User(id: Long, name: String)

object User {
  def empty: User = User(0, "")
}

trait Repository[T] {
  def findById(id: Long): Option[T]
}

def process(x: Int): Int = x * 2
"#;
    let source = create_scala_source(content);

    let result = execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let names: Vec<&str> = skel.declarations.iter().map(|d| d.name.as_str()).collect();

            // Each specific declaration type must be present by name
            assert!(
                names.contains(&"User"),
                "Expected class 'User', found: {:?}",
                names
            );
            assert!(
                names.contains(&"Repository"),
                "Expected trait 'Repository', found: {:?}",
                names
            );
            assert!(
                names.contains(&"process"),
                "Expected function 'process', found: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_finds_definitions() {
    let content = r#"
case class Task(id: Long, title: String) {
  def isComplete: Boolean = true
}

object Task {
  val empty: Task = Task(0, "")
}
"#;
    let source = create_scala_source(content);

    let result = execute_symbols(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<&str> = symbols.iter().map(|s| s.name()).collect();

            // Every expected symbol must appear by name
            assert!(
                names.contains(&"Task"),
                "Expected class 'Task', found: {:?}",
                names
            );
            assert!(
                names.contains(&"id"),
                "Expected parameter 'id', found: {:?}",
                names
            );
            assert!(
                names.contains(&"title"),
                "Expected parameter 'title', found: {:?}",
                names
            );
            assert!(
                names.contains(&"isComplete"),
                "Expected method 'isComplete', found: {:?}",
                names
            );
            assert!(
                names.contains(&"empty"),
                "Expected val 'empty', found: {:?}",
                names
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn parse_class_with_constructor() {
    let content = r#"
class TaskService(
  taskRepo: Repository[Future, Task, Long],
  notificationService: NotificationService
)(implicit ec: ExecutionContext) {
  private val cache: Map[Long, Task] = Map.empty

  def createTask(title: String): Future[Task] = {
    val task = Task(id = generateId(), title = title)
    taskRepo.save(task)
  }

  private def generateId(): Long = System.nanoTime()
}
"#;
    let _source = create_scala_source(content);
    // Parsing succeeded if SourceFile was created
}

#[test]
fn references_query_includes_definition_sites() {
    let content = r#"
def foo(): Int = {
  val bar = 1
  bar
}
"#;
    let source = create_scala_source(content);
    let result = execute_references(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::References { references, .. } => {
            // Find the byte offset of "foo" in "def foo" — the declaring name.
            let def_foo_offset = content.find("def foo").unwrap() + "def ".len();

            let ref_offsets: Vec<usize> =
                references.iter().filter_map(|r| r.byte_offset()).collect();

            assert!(
                ref_offsets.contains(&def_foo_offset),
                "Declaring name 'foo' at byte {} must appear in references, but refs were at offsets: {:?}",
                def_foo_offset,
                ref_offsets
            );
        }
        _ => panic!("Expected References result"),
    }
}

#[test]
fn skeleton_query_finds_methods_inside_class() {
    use act_lite_parser::execute_skeleton;

    let content = r#"
class TaskService {
  def createTask(title: String): Task = ???
  def deleteTask(id: Long): Unit = ???
  private def validate(title: String): Boolean = title.nonEmpty
}
"#;
    let source = create_scala_source(content);
    let result = execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let names: Vec<&str> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                names.contains(&"createTask"),
                "Expected createTask: {:?}",
                names
            );
            assert!(
                names.contains(&"deleteTask"),
                "Expected deleteTask: {:?}",
                names
            );
            assert!(
                names.contains(&"validate"),
                "Expected validate: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_captures_class_parameter_name_not_type() {
    use act_lite_parser::execute_symbols;

    // In `case class Foo(id: Long, name: String)`:
    // class_parameter has both `id` (name) and `Long` (type).
    // Only `id` should be captured as the parameter name.
    let content = r#"case class Foo(id: Long, name: String)"#;
    let source = create_scala_source(content);
    let result = execute_symbols(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<&str> = symbols.iter().map(|s| s.name()).collect();
            assert!(
                names.contains(&"id"),
                "Expected 'id' in symbols: {:?}",
                names
            );
            assert!(
                names.contains(&"name"),
                "Expected 'name' in symbols: {:?}",
                names
            );
            assert!(
                !names.contains(&"Long"),
                "Type 'Long' must not be in symbols: {:?}",
                names
            );
            assert!(
                !names.contains(&"String"),
                "Type 'String' must not be in symbols: {:?}",
                names
            );
        }
        _ => panic!("Expected Symbols result"),
    }
}

#[test]
fn skeleton_query_finds_scala3_enum() {
    use act_lite_parser::execute_skeleton;

    let content = r#"
enum TaskStatus:
  case Pending
  case InProgress
  case Completed
"#;
    let source = create_scala_source(content);
    let result = execute_skeleton(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Skeleton(skel) => {
            let names: Vec<&str> = skel.declarations.iter().map(|d| d.name.as_str()).collect();
            assert!(
                names.contains(&"TaskStatus"),
                "Expected TaskStatus enum: {:?}",
                names
            );
        }
        _ => panic!("Expected Skeleton result"),
    }
}

#[test]
fn symbols_query_finds_scala3_enum_cases() {
    use act_lite_parser::execute_symbols;

    let content = r#"
enum Color:
  case Red
  case Green
  case Blue
"#;
    let source = create_scala_source(content);
    let result = execute_symbols(&source).unwrap();

    match result {
        act_lite_parser::QueryResult::Symbols { symbols } => {
            let names: Vec<&str> = symbols.iter().map(|s| s.name()).collect();
            assert!(names.contains(&"Color"), "Expected Color enum: {:?}", names);
            assert!(names.contains(&"Red"), "Expected Red case: {:?}", names);
            assert!(names.contains(&"Green"), "Expected Green case: {:?}", names);
            assert!(names.contains(&"Blue"), "Expected Blue case: {:?}", names);
        }
        _ => panic!("Expected Symbols result"),
    }
}
