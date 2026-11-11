/* Static analysis permits this read, but execution needs a value. */
int main() {
  int value = value;
  return value;
}
