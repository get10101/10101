enum Direction {
  long,
  short;

  String get nameU => "${name[0].toUpperCase()}${name.substring(1).toLowerCase()}";

  String get keySuffix {
    switch (this) {
      case Direction.long:
        return "Long";
      case Direction.short:
        return "Short";
    }
  }

  Direction opposite() {
    switch (this) {
      case Direction.long:
        return Direction.short;
      case Direction.short:
        return Direction.long;
    }
  }

  // Factory method to convert a String to Direction
  static Direction fromString(String value) {
    switch (value.toLowerCase()) {
      case 'long':
        return Direction.long;
      case 'short':
        return Direction.short;
      default:
        throw ArgumentError('Invalid Direction: $value');
    }
  }
}
