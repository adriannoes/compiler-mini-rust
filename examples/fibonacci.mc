/* Recursive Fibonacci with two calls per recursive case. */
int fibonacci(int n) {
  if (n < 2) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}
int main() {
  return fibonacci(10);
}
