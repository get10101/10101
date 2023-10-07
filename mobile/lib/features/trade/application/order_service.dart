import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/ffi.dart' as rust;

class OrderService {
  Future<String> submitMarketOrder(Leverage leverage, Amount quantity,
      ContractSymbol contractSymbol, Direction direction) async {
    rust.NewOrder order = rust.NewOrder(
        leverage: leverage.leverage,
        quantity: quantity.asDouble(),
        contractSymbol: contractSymbol.toApi(),
        direction: direction.toApi(),
        orderType: const rust.OrderType.market());

    // The problem here is that we have a concurrency issue when sending a payment and trying to open/close a position.
    // The sleep here tries to ensure that we do not process the order matching fee payment from an older order while triggering the next order.
    // This does not fix the underlying issue though: we should block in the protocol that a payment cannot be sent or received at the same time as a subchannel is being added or removed.
    await Future.delayed(const Duration(seconds: 5));

    return await rust.api.submitOrder(order: order);
  }

  Future<List<Order>> fetchOrders() async {
    List<rust.Order> apiOrders = await rust.api.getOrders();
    List<Order> orders = apiOrders.map((order) => Order.fromApi(order)).toList();

    return orders;
  }

  Future<Order?> fetchAsyncOrder() async {
    rust.Order? order = await rust.api.getAsyncOrder();

    if (order == null) {
      return null;
    }

    return Order.fromApi(order);
  }
}
