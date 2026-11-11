/* Expressions for AST inspection. */
int arithmetic() {
  return (007 + 2) * -3 - 4 / 2 % 2;
}
bool logic() {
  return !false || true && 1 + 2 * 3 < 8 == true;
}
int assignment() {
  return left = right = 123456789012345678901234567890;
}
