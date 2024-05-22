import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/ffi.dart' as rust;

class OrderService {
  Future<String> submitMarketOrder(Leverage leverage, Usd quantity, ContractSymbol contractSymbol,
      Direction direction, bool stable) async {
    rust.NewOrder order = rust.NewOrder(
        leverage: leverage.leverage,
        quantity: quantity.asDouble(),
        contractSymbol: contractSymbol.toApi(),
        direction: direction.toApi(),
        orderType: const rust.OrderType.market(),
        stable: stable);

    return await rust.api.submitOrder(order: order);
  }

  Future<String> submitChannelOpeningMarketOrder(
      Leverage leverage,
      Usd quantity,
      ContractSymbol contractSymbol,
      Direction direction,
      bool stable,
      Amount coordinatorReserve,
      Amount traderReserve) async {
    rust.NewOrder order = rust.NewOrder(
        leverage: leverage.leverage,
        quantity: quantity.asDouble(),
        contractSymbol: contractSymbol.toApi(),
        direction: direction.toApi(),
        orderType: const rust.OrderType.market(),
        stable: stable);

    return await rust.api.submitChannelOpeningOrder(
        order: order,
        coordinatorReserve: coordinatorReserve.sats,
        traderReserve: traderReserve.sats);
  }

  // starts a process to watch for funding an address before creating the order
  // returns the address to watch for
  Future<String> submitUnfundedChannelOpeningMarketOrder(
      Leverage leverage,
      Usd quantity,
      ContractSymbol contractSymbol,
      Direction direction,
      bool stable,
      Amount coordinatorReserve,
      Amount traderReserve,
      Amount margin) async {
    rust.NewOrder order = rust.NewOrder(
        leverage: leverage.leverage,
        quantity: quantity.asDouble(),
        contractSymbol: contractSymbol.toApi(),
        direction: direction.toApi(),
        orderType: const rust.OrderType.market(),
        stable: stable);

    var address = await rust.api.getNewAddress();

    await rust.api.submitUnfundedChannelOpeningOrder(
        fundingAddress: address,
        order: order,
        coordinatorReserve: coordinatorReserve.sats,
        traderReserve: traderReserve.sats,
        estimatedMargin: margin.sats);
    return address;
  }

  Future<List<Order>> fetchOrders() async {
    List<rust.Order> apiOrders = await rust.api.getOrders();
    List<Order> orders = apiOrders.map((order) => Order.fromApi(order)).toList();

    return orders;
  }
}
