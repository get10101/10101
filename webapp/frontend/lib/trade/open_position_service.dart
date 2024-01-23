import 'dart:convert';
import 'package:flutter/cupertino.dart';
import 'package:get_10101/common/http_client.dart';
import 'package:get_10101/common/model.dart';

class OpenPositionsService {
  const OpenPositionsService();

  static Future<List<Position>> fetchOpenPositions() async {
    final response = await HttpClientManager.instance.get(Uri(path: '/api/positions'));

    if (response.statusCode == 200) {
      final List<dynamic> jsonData = jsonDecode(response.body);
      return jsonData.map((positionData) => Position.fromJson(positionData)).toList();
    } else {
      throw FlutterError("Could not fetch positions");
    }
  }
}

class Position {
  final Leverage leverage;
  final Usd quantity;
  final String contractSymbol;
  final String direction;
  final Usd averageEntryPrice;
  final Usd liquidationPrice;
  final String positionState;
  final Amount collateral;
  final DateTime expiry;
  final DateTime updated;
  final DateTime created;

  Position({
    required this.leverage,
    required this.quantity,
    required this.contractSymbol,
    required this.direction,
    required this.averageEntryPrice,
    required this.liquidationPrice,
    required this.positionState,
    required this.collateral,
    required this.expiry,
    required this.updated,
    required this.created,
  });

  factory Position.fromJson(Map<String, dynamic> json) {
    return Position(
      leverage: Leverage(json['leverage'] as double),
      quantity: Usd(json['quantity'] as double),
      contractSymbol: json['contract_symbol'] as String,
      direction: json['direction'] as String,
      averageEntryPrice: Usd(json['average_entry_price'] as double),
      liquidationPrice: Usd(json['liquidation_price'] as double),
      positionState: json['position_state'] as String,
      collateral: Amount(json['collateral']),
      expiry: DateTime.parse(json['expiry'] as String),
      updated: DateTime.parse(json['updated'] as String),
      created: DateTime.parse(json['created'] as String),
    );
  }
}
