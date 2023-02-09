import '../../../bridge_generated/bridge_definitions.dart';

enum Direction { buy, sell }

extension DirectionExt on Direction {
  Position intoPosition() {
    switch (this) {
      case Direction.buy:
        return Position.Long;
      case Direction.sell:
        return Position.Short;
    }
  }

  String get nameU => "${name[0].toUpperCase()}${name.substring(1).toLowerCase()}";
}
