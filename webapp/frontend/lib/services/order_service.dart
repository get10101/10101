import 'dart:convert';
import 'package:flutter/cupertino.dart';
import 'package:get_10101/common/contract_symbol.dart';
import 'package:get_10101/common/direction.dart';
import 'package:get_10101/common/http_client.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/order_type.dart';

class OrderService {
  const OrderService();

  Future<List<Order>> fetchOrders() async {
    final response = await HttpClientManager.instance.get(Uri(path: '/api/orders'));

    if (response.statusCode == 200) {
      final List<dynamic> jsonData = jsonDecode(response.body);
      return jsonData.map((orderData) => Order.fromJson(orderData)).toList();
    } else {
      throw FlutterError("Could not fetch orders");
    }
  }
}

class Order {
  final String id; //: Uuid,
  final Leverage leverage; //: f32,
  final Usd quantity; //: f32,
  final Usd? price; //: f32,
  final ContractSymbol contractSymbol; //: ContractSymbol,
  final Direction direction; //: Direction,
  final OrderType orderType; //: OrderType,
  // TODO: define a state
  final OrderState state; //: OrderState,
  final DateTime creationTimestamp; //: OffsetDateTime,
  // TODO: define failure reason
  final String? failureReason; //: Option<FailureReason>,

  Order(
      {required this.id,
      required this.leverage,
      required this.quantity,
      required this.price,
      required this.contractSymbol,
      required this.direction,
      required this.orderType,
      required this.state,
      required this.creationTimestamp,
      required this.failureReason});

  factory Order.fromJson(Map<String, dynamic> json) {
    return Order(
      id: json['id'] as String,
      leverage: Leverage(json['leverage'] as double),
      quantity: Usd(json['quantity'] as double),
      price: json['price'] != null ? Usd(json['price'] as double) : null,
      contractSymbol: ContractSymbol.btcusd,
      direction: Direction.fromString(json['direction']),
      creationTimestamp: DateTime.parse(json['creation_timestamp'] as String),
      orderType: OrderType.fromString(json['order_type'] as String),
      state: OrderState.fromString(json['state'] as String), //json['state'] as String,
      failureReason: json['failure_reason'], // json['failure_reason'],
    );
  }
}

enum OrderState {
  initial,
  rejected,
  open,
  filling,
  failed,
  filled,
  unknown;

  String get asString {
    switch (this) {
      case OrderState.initial:
        return "Initial";
      case OrderState.rejected:
        return "Rejected";
      case OrderState.open:
        return "Open";
      case OrderState.filling:
        return "Filling";
      case OrderState.failed:
        return "Failed";
      case OrderState.filled:
        return "Filled";
      case OrderState.unknown:
        return "Unknown";
    }
  }

  static OrderState fromString(String value) {
    switch (value.toLowerCase()) {
      case 'initial':
        return OrderState.initial;
      case 'rejected':
        return OrderState.rejected;
      case 'open':
        return OrderState.open;
      case 'filling':
        return OrderState.filling;
      case 'filled':
        return OrderState.filled;
      case 'failed':
        return OrderState.failed;
      default:
        throw ArgumentError('Invalid OrderState: $json');
    }
  }
}
