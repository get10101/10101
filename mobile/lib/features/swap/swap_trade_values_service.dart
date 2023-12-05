// Copy of TradeValuesService but with floored dollar amounts
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/ffi.dart' as rust;

class SwapTradeValuesService extends TradeValuesService {
  @override
  Amount? calculateQuantity(
      {required double? price, required Amount? margin, required Leverage leverage, dynamic hint}) {
    if (price == null || margin == null) {
      return null;
    } else {
      final quantity = rust.api
          .calculateQuantity(price: price, margin: margin.sats, leverage: leverage.leverage);
      return Amount(quantity.floor());
    }
  }
}
