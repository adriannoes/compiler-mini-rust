/* Braced and unbraced control flow for AST inspection. */
int choose(bool first, bool second) {
  int result = 0;
  if (first)
    if (second)
      result = 1;
    else
      result = 2;
  while (result < 3) {
    result = result + 1;
  }
  return result;
}
