/* Arithmetic, assignment, and lexical scopes. */
int main() {
  int result;
  int base = 7;
  {
    int base = 6;
    result = base * 7;
  }
  return result + base - 7;
}
