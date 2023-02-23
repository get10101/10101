class Leverage {
  double leverage;

  Leverage(this.leverage);

  String formatted() => "x${leverage % 1 == 0 ? leverage.toInt().toString() : leverage.toString()}";
}
