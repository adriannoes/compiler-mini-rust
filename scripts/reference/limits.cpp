#include "bytecode_compiler.hpp"
#include "bytecode_optimizer.hpp"
#include "interpreter.hpp"
#include "lexer.hpp"
#include "parser.hpp"
#include "vm.hpp"

#include <fstream>
#include <iostream>
#include <iterator>

template <class Execute>
void print_result(Execute execute) {
  try {
    const auto value = execute();
    if (std::holds_alternative<std::monostate>(value)) {
      std::cout << "ok void\n";
    } else {
      std::cout << "ok " << minic::format_value(value) << '\n';
    }
  } catch (const minic::RuntimeError& error) {
    std::cout << error.location.line << ':' << error.location.column << ' ' << error.what() << '\n';
  }
}

int main(int argc, char** argv) {
  for (int i = 1; i < argc; ++i) {
    std::ifstream input(argv[i], std::ios::binary);
    if (!input) {
      std::cerr << "Cannot open " << argv[i] << '\n';
      return 1;
    }
    const std::string source{std::istreambuf_iterator<char>(input), {}};
    const auto tree = minic::parse(minic::lex(source));
    const auto original = minic::compile_bytecode(tree);
    const auto optimized = minic::optimize_bytecode(original);
    for (std::size_t steps = 1; steps <= 80; ++steps) {
      for (std::size_t depth : {1, 2, 4}) {
        const minic::ExecutionLimits limits{steps, depth};
        print_result([&] { return minic::interpret(tree, "main", limits); });
        print_result([&] { return minic::execute_bytecode(original, "main", limits); });
        print_result([&] { return minic::execute_bytecode(optimized, "main", limits); });
      }
    }
  }
}
