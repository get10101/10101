import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/ffi.dart' as rust;

class PositionService {
  Future<List<Position>> fetchPositions() async {
    List<rust.Position> apiPositions = await rust.api.getPositions();
    List<Position> positions = apiPositions.map((position) => Position.fromApi(position)).toList();

    return positions;
  }
}
