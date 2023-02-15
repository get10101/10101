import '../../../bridge_generated/bridge_definitions.dart' as rust;

enum Direction { long, short }

extension DirectionExt on Direction {
  rust.Direction toApi() {
    switch (this) {
      case Direction.long:
        return rust.Direction.Long;
      case Direction.short:
        return rust.Direction.Short;
    }
  }

  String get nameU => "${name[0].toUpperCase()}${name.substring(1).toLowerCase()}";
}
