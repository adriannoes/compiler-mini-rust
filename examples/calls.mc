/* Declarations and named calls for AST inspection. */
int add(int left, int right) {
  int total = left + right;
  return total;
}
int main() {
  int result;
  result = add(1, add(2, 3));
  bool ready = true;
  ;
  return result;
}
