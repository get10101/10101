import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/balance.dart';
import 'package:get_10101/common/payment.dart';
import 'package:http/http.dart' as http;

class WalletService {
  const WalletService();

  Future<Balance> getBalance() async {
    // TODO(holzeis): this should come from the config
    const port = "3001";
    const host = "localhost";

    final response = await http.get(Uri.http('$host:$port', '/api/balance'));

    if (response.statusCode == 200) {
      return Balance.fromJson(jsonDecode(response.body) as Map<String, dynamic>);
    } else {
      throw FlutterError("Failed to fetch balance");
    }
  }

  Future<String> getNewAddress() async {
    // TODO(holzeis): this should come from the config
    const port = "3001";
    const host = "localhost";

    final response = await http.get(Uri.http('$host:$port', '/api/newaddress'));

    if (response.statusCode == 200) {
      return response.body;
    } else {
      throw FlutterError("Failed to fetch new address");
    }
  }

  Future<void> sendPayment(String address, Amount amount, Amount fee) async {
    // TODO(holzeis): this should come from the config
    const port = "3001";
    const host = "localhost";

    final response = await http.post(Uri.http('$host:$port', '/api/sendpayment'),
        headers: <String, String>{
          'Content-Type': 'application/json; charset=UTF-8',
        },
        body: jsonEncode(
            <String, dynamic>{'address': address, 'amount': amount.sats, 'fee': fee.sats}));

    if (response.statusCode != 200) {
      throw FlutterError("Failed to send payment");
    }
  }

  Future<List<OnChainPayment>> getOnChainPaymentHistory() async {
    // TODO(holzeis): this should come from the config
    const port = "3001";
    const host = "localhost";

    final response = await http.get(Uri.http('$host:$port', '/api/history'));

    if (response.statusCode == 200) {
      List<OnChainPayment> history = [];
      Iterable list = json.decode(response.body);
      for (int i = 0; i < list.length; i++) {
        history.add(OnChainPayment.fromJson(list.elementAt(i)));
      }
      return history;
    } else {
      throw FlutterError("Failed to fetch onchain payment history");
    }
  }
}
