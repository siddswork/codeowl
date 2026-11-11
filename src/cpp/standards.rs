//! One test per C++ standard: the syntax that standard introduced, and the
//! symbols the pack must still find around it. The samples are small and
//! hand-written, so they show *which* syntax is read, not how often it
//! occurs (`experiments/exp-08-cpp-tree-sitter-or-compiler.md`, "Standards").
//!
//! What is asserted is extraction, not a clean parse: where the grammar
//! does not know a construct, the test says so and checks that the
//! declaration around it survives.

use super::{extract_file, parse_error_count};
use crate::symbol::{ExtractedSymbol, SymbolKind};

fn extract(src: &str) -> Vec<ExtractedSymbol> {
    extract_file(src, "s.h")
}

/// Every id in `expected` is present, in any order, with the given kind.
#[track_caller]
fn has(syms: &[ExtractedSymbol], expected: &[(&str, SymbolKind)]) {
    for (id, kind) in expected {
        let id = format!("s.h::{id}");
        let found = syms.iter().find(|s| s.id == id).unwrap_or_else(|| {
            let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
            panic!("no symbol `{id}`; have {ids:?}")
        });
        assert_eq!(found.kind, *kind, "{id}");
    }
}

use SymbolKind::{Callable, Container, Value};

#[test]
fn cpp98() {
    let src = r#"
namespace geo { namespace detail { int helper(int); } }
template <class T, int N = 4> class Buffer { public: T data[N]; T& operator[](int i) { return data[i]; } };
template <> class Buffer<bool, 1> { public: bool b; };
class Shape { public: virtual ~Shape() {} virtual double area() const = 0; protected: int id_; private: Shape& operator=(const Shape&); };
class Circle : public Shape, private std::vector<int> {
 public:
  explicit Circle(double r) : r_(r) {}
  double area() const throw() { return 3.14 * r_ * r_; }
  operator double() const { return r_; }
  friend bool operator==(const Circle&, const Circle&);
  static int count;
 private:
  double r_;
};
int Circle::count = 0;
typedef int (*Callback)(void*, const char*);
enum Color { Red, Green = 5 };
union U { int i; float f; };
struct Bits { unsigned a : 3; unsigned b : 5; };
extern "C" { int c_api(int); }
int geo::detail::helper(int x) { try { return x; } catch (...) { return 0; } }
template <class T> typename T::value_type first(const T& c) { return *c.begin(); }
"#;
    let syms = extract(src);
    has(
        &syms,
        &[
            ("helper", Callable),
            ("Buffer", Container),
            ("Buffer::operator[]", Callable),
            ("Shape", Container),
            ("Shape::~Shape", Callable),
            ("Shape::area", Callable),
            ("Circle", Container),
            ("Circle::Circle", Callable),
            ("Circle::operator double", Callable),
            ("Circle::count", Value),
            ("Callback", Value),
            ("Color", Container),
            ("Color::Green", Value),
            ("U", Container),
            ("Bits::a", Value),
            ("c_api", Callable),
            ("geo::detail::helper", Callable),
            ("first", Callable),
        ],
    );
    assert_eq!(
        syms.iter()
            .find(|s| s.id == "s.h::Circle")
            .unwrap()
            .signature,
        "class Circle : public Shape, private std::vector<int>"
    );
    // A friend declaration names something declared elsewhere; it is not a
    // member and is not extracted.
    assert!(!syms.iter().any(|s| s.id == "s.h::Circle::operator=="));
    assert_eq!(parse_error_count(src), 0);

    // Known gap: a typedef of a pointer to a `const` member function does
    // not parse. The alias is still found.
    let gap = "typedef double (Shape::*AreaFn)() const;\nint after;\n";
    assert!(parse_error_count(gap) > 0);
    has(&extract(gap), &[("after", Value)]);
}

#[test]
fn cpp11() {
    let src = r#"
enum class Mode : unsigned char { Read, Write };
template <typename... Ts> struct Tuple {};
template <typename T> using Ptr = std::unique_ptr<T>;
struct Base { virtual void f() noexcept; virtual ~Base() = default; Base(const Base&) = delete; Base() = default; };
struct Derived final : Base { void f() noexcept override; int x{3}; alignas(16) char buf[16]; };
constexpr int square(int x) { return x * x; }
auto add(int a, int b) -> decltype(a + b) { return a + b; }
template <typename T> void sink(T&& v) { auto l = [&, v](int a) mutable -> int { return a; }; static_assert(sizeof(T) > 0, "x"); }
[[noreturn]] void die();
struct W { W(std::initializer_list<int>); W(W&&) noexcept; W& operator=(W&&) & = default; explicit operator bool() const; };
thread_local int tls = 0;
long long operator"" _km(unsigned long long v) { return v; }
void g() { for (auto& x : {1, 2}) { (void)x; } int* p = nullptr; auto r = R"(raw)"; }
"#;
    let syms = extract(src);
    has(
        &syms,
        &[
            ("Mode", Container),
            ("Mode::Write", Value),
            ("Tuple", Container),
            ("Ptr", Value),
            ("Base::~Base", Callable),
            ("Derived", Container),
            ("Derived::f", Callable),
            ("Derived::x", Value),
            ("Derived::buf", Value),
            ("square", Callable),
            ("add", Callable),
            ("sink", Callable),
            ("die", Callable),
            ("W::operator=", Callable),
            ("W::operator bool", Callable),
            ("tls", Value),
            ("g", Callable),
        ],
    );
    let sig = |id: &str| syms.iter().find(|s| s.id == id).unwrap().signature.clone();
    assert_eq!(sig("s.h::Derived"), "struct Derived final : Base");
    assert_eq!(sig("s.h::add"), "auto add(int a, int b) -> decltype(a + b)");
    // The brace initialiser is not part of a member's signature.
    assert_eq!(sig("s.h::Derived::x"), "int x");
    assert_eq!(parse_error_count(src), 0);
}

#[test]
fn cpp14() {
    let src = r#"
template <typename T> constexpr T pi = T(3.1415926535897932385L);
auto deduced(int x) { return x * 2; }
decltype(auto) fwd(int& x) { return (x); }
[[deprecated("use g")]] void old();
constexpr int fact(int n) { int r = 1; for (int i = 2; i <= n; ++i) r *= i; return r; }
void h() { auto gl = [](auto a, auto b) { return a + b; }; int big = 1'000'000; int bin = 0b1010; }
"#;
    has(
        &extract(src),
        &[
            ("pi", Value),
            ("deduced", Callable),
            ("fwd", Callable),
            ("old", Callable),
            ("fact", Callable),
            ("h", Callable),
        ],
    );
    assert_eq!(parse_error_count(src), 0);
}

#[test]
fn cpp17() {
    let src = r#"
namespace a::b::c { inline int v = 1; }
template <typename... Ts> auto sum(Ts... ts) { return (ts + ... + 0); }
template <typename T> struct Box { T v; Box(T t) : v(t) {} };
template <auto N> struct Val { static constexpr auto value = N; };
[[nodiscard]] std::optional<int> find(std::string_view s) noexcept;
struct S { inline static int count = 0; [[maybe_unused]] int pad; };
template <typename T> auto size_of(const T& t) { if constexpr (sizeof(T) > 4) { return 8; } else { return 4; } }
void k(std::map<int, int>& m) { if (auto it = m.find(1); it != m.end()) { auto& [key, val] = *it; } }
"#;
    has(
        &extract(src),
        &[
            ("v", Value),
            ("sum", Callable),
            ("Box", Container),
            ("Box::Box", Callable),
            ("Val::value", Value),
            ("find", Callable),
            ("S::count", Value),
            ("S::pad", Value),
            ("size_of", Callable),
            ("k", Callable),
        ],
    );
    assert_eq!(parse_error_count(src), 0);

    // Known gap: a pack expansion in a `using` declaration does not parse.
    // The class holding it is still found.
    let gap = "template <class... Ts> struct overloaded : Ts... { using Ts::operator()...; };\n";
    assert!(parse_error_count(gap) > 0);
    has(&extract(gap), &[("overloaded", Container)]);
}

#[test]
fn cpp20() {
    let src = r#"
template <typename T> concept Number = std::integral<T> || std::floating_point<T>;
template <typename T> concept Hashable = requires(T a) { { a.hash() } -> std::convertible_to<unsigned long>; };
template <Number T> T twice(T x) { return x + x; }
template <typename T> requires Number<T> T thrice(T x) { return x * 3; }
auto abbreviated(Number auto x, std::span<const int> s) { return x; }
struct P { int x, y; auto operator<=>(const P&) const = default; bool operator==(const P&) const = default; };
struct Agg { int a; int b = 2; [[no_unique_address]] P p; };
constinit int ci = 1;
consteval int cube(int x) { return x * x * x; }
struct Task { struct promise_type { Task get_return_object() { return {}; } void return_void() {} }; };
Task coro() { co_await std::suspend_never{}; co_return; }
template <typename T> struct Opt { explicit(sizeof(T) > 4) Opt(T); };
void m() { Agg a{.a = 1, .b = 3}; auto l = []<typename T>(T t) requires Number<T> { return t; }; }
"#;
    let syms = extract(src);
    has(
        &syms,
        &[
            ("Number", Value),
            ("Hashable", Value),
            ("twice", Callable),
            ("thrice", Callable),
            ("abbreviated", Callable),
            ("P::operator<=>", Callable),
            ("P::operator==", Callable),
            ("Agg::b", Value),
            ("Agg::p", Value),
            ("ci", Value),
            ("cube", Callable),
            ("Task::promise_type", Container),
            ("Task::promise_type::return_void", Callable),
            ("coro", Callable),
            ("Opt::Opt", Callable),
            ("m", Callable),
        ],
    );
    // The requires-clause is part of the signature.
    assert_eq!(
        syms.iter()
            .find(|s| s.id == "s.h::thrice")
            .unwrap()
            .signature,
        "template <typename T> requires Number<T> T thrice(T x)"
    );
    assert_eq!(parse_error_count(src), 0);
}

#[test]
fn cpp20_modules() {
    // The grammar knows nothing of modules. The pack blanks the `export`
    // keyword and the module lines, so what is exported is still extracted.
    let src = r#"module;
#include <string>
export module shapes:circle;
import :base;
import <vector>;
export namespace shapes { class Circle { public: double area() const; }; }
export template <typename T> T ident(T t) { return t; }
export int exported_fn();
module :private;
double shapes::Circle::area() const { return 0; }
"#;
    let syms = extract_file(src, "shapes.cppm");
    let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "shapes.cppm::Circle",
            "shapes.cppm::Circle::area",
            "shapes.cppm::ident",
            "shapes.cppm::exported_fn",
            "shapes.cppm::shapes::Circle::area",
        ]
    );
    // Known limit: the signature starts at the declaration the grammar saw,
    // so the blanked `export` in front of it is not in it. Making a
    // declaration exported, or not, therefore does not move its hashes.
    assert_eq!(syms[2].signature, "template <typename T> T ident(T t)");
    assert_eq!(parse_error_count(src), 0);

    // An `export { … }` block: with the keyword blanked it is a bare block,
    // and the declarations inside it are read like any others.
    let block = "export {\nint in_block();\nclass Inside {};\n}\nint after;\n";
    let syms = extract_file(block, "m.cppm");
    let ids: Vec<&str> = syms.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["m.cppm::in_block", "m.cppm::Inside", "m.cppm::after"]);
}

#[test]
fn cpp23() {
    // The explicit object parameter is blanked so the method parses.
    let src = r#"
struct Self { template <typename S> auto&& get(this S&& self) { return self.v; } int v; void by_value(this Self self); };
struct Grid { int operator[](std::size_t r, std::size_t c) const { return 0; } static int operator()(int x) { return x; } };
extern "C++" { void cxx(); }
"#;
    let syms = extract(src);
    has(
        &syms,
        &[
            ("Self", Container),
            ("Self::get", Callable),
            ("Self::v", Value),
            ("Self::by_value", Callable),
            ("Grid::operator[]", Callable),
            ("Grid::operator()", Callable),
            ("cxx", Callable),
        ],
    );
    assert_eq!(
        syms.iter()
            .find(|s| s.id == "s.h::Self::by_value")
            .unwrap()
            .signature,
        "void by_value(this Self self)"
    );
    assert_eq!(parse_error_count(src), 0);

    // Known gaps, all inside function bodies: `if consteval` and a `static`
    // lambda do not parse. The functions around them are still found.
    let gap = "constexpr int ce(int x) { if consteval { return x; } else { return x + 1; } }\n\
               void n() { auto l = [] static { return 1; }; }\nint after;\n";
    assert!(parse_error_count(gap) > 0);
    has(
        &extract(gap),
        &[("ce", Callable), ("n", Callable), ("after", Value)],
    );
}

#[test]
fn cpp26() {
    // `= delete("reason")` and contract clauses are blanked.
    let src = r#"
struct D { D() = delete("use make()"); int kept; };
constexpr int with_contract(int x) pre(x > 0) post(r: r > 0) { return x; }
void q(std::tuple<int, int> t) { auto [_, _] = t; }
"#;
    let syms = extract(src);
    has(
        &syms,
        &[
            ("D", Container),
            ("D::D", Callable),
            ("D::kept", Value),
            ("with_contract", Callable),
            ("q", Callable),
        ],
    );
    assert_eq!(
        syms.iter()
            .find(|s| s.id == "s.h::with_contract")
            .unwrap()
            .signature,
        "constexpr int with_contract(int x) pre(x > 0) post(r: r > 0)"
    );
    assert_eq!(parse_error_count(src), 0);

    // Known gap: pack indexing (`Ts...[0]`) does not parse. The function and
    // the class around it are still found.
    let gap = "template <typename... Ts> struct Pack { template <std::size_t I> using at = Ts...[I]; };\n\
               template <typename... Ts> auto first(Ts... ts) { return ts...[0]; }\n";
    assert!(parse_error_count(gap) > 0);
    has(&extract(gap), &[("Pack", Container), ("first", Callable)]);
}
