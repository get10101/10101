import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/ffi.dart' as rust;

class PositionService {
  Future<List<Position>> fetchPositions() async {
    List<rust.Position> apiPositions = await rust.api.getPositions();
    List<Position> positions = apiPositions.map((position) => Position.fromApi(position)).toList();

    return positions;
  }

  /// Returns the pnl in sat
  int? calculatePnl(Position position, double askPrice, double bidPrice) {
    final closingPrice = rust.Price(
      bid: bidPrice,
      ask: askPrice,
    );
    return rust.api.calculatePnl(
        openingPrice: position.averageEntryPrice,
        closingPrice: closingPrice,
        quantity: position.quantity.asDouble(),
        leverage: position.leverage.leverage,
        direction: position.direction.toApi());
  }
}
