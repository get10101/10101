import 'package:get_10101/bridge_generated/bridge_definitions.dart' as rust;

enum Direction {
  long,
  short;

  rust.Direction toApi() {
    switch (this) {
      case Direction.long:
        return rust.Direction.Long;
      case Direction.short:
        return rust.Direction.Short;
    }
  }

  static Direction fromApi(rust.Direction direction) {
    switch (direction) {
      case rust.Direction.Long:
        return Direction.long;
      case rust.Direction.Short:
        return Direction.short;
    }
  }

  String get nameU => "${name[0].toUpperCase()}${name.substring(1).toLowerCase()}";
}
