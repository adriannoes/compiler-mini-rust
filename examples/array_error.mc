/* An initialized element does not give other elements a value. */
int main() {
  int values[2];
  values[0] = 4;
  return values[1];
}
