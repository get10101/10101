import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:http/http.dart' as http;

class OrderId {
  final String orderId;

  const OrderId({required this.orderId});

  factory OrderId.fromJson(Map<String, dynamic> json) {
    return switch (json) {
      {
        'id': String orderId,
      } =>
        OrderId(orderId: orderId),
      _ => throw const FormatException('Failed to parse order id.'),
    };
  }
}

class NewOrderService {
  const NewOrderService();

  static Future<String> postNewOrder(Leverage leverage, Usd quantity, bool isLong) async {
    // TODO(holzeis): this should come from the config
    const port = "3001";
    const host = "localhost";

    final response = await http.post(Uri.http('$host:$port', '/api/orders'),
        headers: <String, String>{
          'Content-Type': 'application/json; charset=UTF-8',
        },
        body: jsonEncode(<String, dynamic>{
          'leverage': leverage.asDouble,
          'quantity': quantity.asDouble,
          'direction': isLong ? "Long" : "Short",
        }));

    if (response.statusCode == 200) {
      return OrderId.fromJson(jsonDecode(response.body) as Map<String, dynamic>).orderId;
    } else {
      throw FlutterError("Failed to post new order. Response ${response.body}");
    }
  }
}
