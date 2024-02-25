class Leverage {
  double leverage;

  Leverage(this.leverage);

  String formatted() => "x${leverage % 1 == 0 ? leverage.toInt().toString() : leverage.toString()}";
  String formattedReverse() =>
      "${leverage % 1 == 0 ? leverage.toInt().toString() : leverage.toString()}x";

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Leverage && runtimeType == other.runtimeType && leverage == other.leverage;

  @override
  int get hashCode => leverage.hashCode;
}
