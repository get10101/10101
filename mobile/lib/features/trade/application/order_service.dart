import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/features/trade/domain/response_status.dart';
import 'package:get_10101/ffi.dart' as rust;

import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/api_response.dart';

class OrderService {
  Future<ApiResponse> submitMarketOrder(Leverage leverage, double quantity,
      ContractSymbol contractSymbol, Direction direction) async {
    rust.NewOrder order = rust.NewOrder(
        leverage: leverage.leverage,
        quantity: quantity,
        contractSymbol: contractSymbol.toApi(),
        direction: direction.toApi(),
        orderType: const rust.OrderType.market());

    try {
      await rust.api.submitOrder(order: order);
      return ApiResponse(status: ResponseStatus.success);
    } on FfiException catch (error) {
      return ApiResponse(status: ResponseStatus.failure, errorText: error.message);
    }
  }
}
