/* Lexer example. */
void count(int limit, bool enabled) {
  int n = -1 + 2 * 3 / 4 % 5;
  while (enabled && n < limit) {
    n = n + 1;
    if (n == 2 || n != 3) {
      enabled = !false;
    } else {
      enabled = n <= limit && n >= 0 && n > 0;
    }
  }
  return;
}
int main() {
  count(5, true);
  return 0;
}
