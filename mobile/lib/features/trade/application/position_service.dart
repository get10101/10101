import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/ffi.dart' as rust;

import '../domain/price.dart';

class PositionService {
  Future<List<Position>> fetchPositions() async {
    List<rust.Position> apiPositions = await rust.api.getPositions();
    List<Position> positions = apiPositions.map((position) => Position.fromApi(position)).toList();

    return positions;
  }

  /// Returns the pnl in sat
  int calculatePnl(Position position, Price price) {
    return rust.api.calculatePnl(
        openingPrice: position.averageEntryPrice,
        closingPrice: rust.Price(
          bid: price.bid,
          ask: price.ask,
        ),
        quantity: position.quantity,
        leverage: position.leverage.leverage,
        direction: position.direction.toApi());
  }
}
