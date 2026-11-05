class Account {
  int balance = 0;
  void deposit(int amount) { balance += amount; }
}

int double_it(int v) => v * 2;

void main() {
  print(double_it(2));
}
