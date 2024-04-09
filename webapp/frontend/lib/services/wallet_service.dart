import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:get_10101/common/http_client.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/balance.dart';
import 'package:get_10101/common/payment.dart';

class WalletService {
  const WalletService();

  Future<Balance> getBalance() async {
    final response = await HttpClientManager.instance.get(Uri(path: '/api/balance'));

    if (response.statusCode == 200) {
      return Balance.fromJson(jsonDecode(response.body) as Map<String, dynamic>);
    } else {
      throw FlutterError("Failed to fetch balance");
    }
  }

  Future<String> getNewAddress() async {
    final response = await HttpClientManager.instance.get(Uri(path: '/api/newaddress'));

    if (response.statusCode == 200) {
      return response.body;
    } else {
      throw FlutterError("Failed to fetch new address");
    }
  }

  Future<void> sendPayment(String address, Amount amount, Amount fee) async {
    final response = await HttpClientManager.instance.post(Uri(path: '/api/sendpayment'),
        headers: <String, String>{
          'Content-Type': 'application/json; charset=UTF-8',
        },
        body: jsonEncode(
            <String, dynamic>{'address': address, 'amount': amount.sats, 'fee': fee.sats}));

    if (response.statusCode != 200) {
      throw FlutterError(response.body);
    }
  }

  Future<void> sync() async {
    final response =
        await HttpClientManager.instance.post(Uri(path: '/api/sync'), headers: <String, String>{
      'Content-Type': 'application/json; charset=UTF-8',
    });

    if (response.statusCode != 200) {
      throw FlutterError("Failed to sync wallet");
    }
  }

  Future<List<OnChainPayment>> getOnChainPaymentHistory() async {
    final response = await HttpClientManager.instance.get(Uri(path: '/api/history'));

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
