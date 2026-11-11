/* Nested scopes and a call before its definition. */
int main() {
  int value = later(1);
  {
    int copy = value;
    int value = copy;
    value = value + 1;
  }
  return value;
}
int later(int input) {
  return input;
}
