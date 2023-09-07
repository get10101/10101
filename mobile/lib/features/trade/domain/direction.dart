import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

enum Direction {
  long,
  short;

  bridge.Direction toApi() {
    switch (this) {
      case Direction.long:
        return bridge.Direction.Long;
      case Direction.short:
        return bridge.Direction.Short;
    }
  }

  static Direction fromApi(bridge.Direction direction) {
    switch (direction) {
      case bridge.Direction.Long:
        return Direction.long;
      case bridge.Direction.Short:
        return Direction.short;
    }
  }

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
}
