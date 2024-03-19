import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/ffi.dart' as rust;

class OrderService {
  Future<String> submitMarketOrder(Leverage leverage, Usd quantity, ContractSymbol contractSymbol,
      Direction direction, bool stable, Amount margin, bool isMarginOrder) async {
    Usd updatedQuantity = quantity;
    rust.OrderType orderType = const rust.OrderType.market();

    if (isMarginOrder) {
      // just to be on the safe side we set quantity to 0
      updatedQuantity = Usd.zero();
      orderType = const rust.OrderType.margin();
    }

    rust.NewOrder order = rust.NewOrder(
        leverage: leverage.leverage,
        quantity: updatedQuantity.asDouble(),
        contractSymbol: contractSymbol.toApi(),
        direction: direction.toApi(),
        orderType: orderType,
        stable: stable,
        marginSats: margin.asDouble());

    return await rust.api.submitOrder(order: order);
  }

  Future<String> submitChannelOpeningMarketOrder(
      Leverage leverage,
      Usd quantity,
      ContractSymbol contractSymbol,
      Direction direction,
      bool stable,
      Amount coordinatorReserve,
      Amount traderReserve,
      Amount margin,
      bool isMarginOrder) async {
    Usd updatedQuantity = quantity;
    rust.OrderType orderType = const rust.OrderType.market();

    if (isMarginOrder) {
      // just to be on the safe side we set quantity to 0
      updatedQuantity = Usd.zero();
      orderType = const rust.OrderType.margin();
    }

    rust.NewOrder order = rust.NewOrder(
        leverage: leverage.leverage,
        quantity: updatedQuantity.asDouble(),
        contractSymbol: contractSymbol.toApi(),
        direction: direction.toApi(),
        orderType: orderType,
        stable: stable,
        marginSats: margin.asDouble());

    return await rust.api.submitChannelOpeningOrder(
        order: order,
        coordinatorReserve: coordinatorReserve.sats,
        traderReserve: traderReserve.sats);
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
